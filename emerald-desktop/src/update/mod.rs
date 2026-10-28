//! Updates from a release feed. A thread checks the feed a little after
//! startup and every few hours, downloads a newer build for this platform
//! and checks its signature against the release key before anything is
//! kept. The app then offers a restart, which installs it.
//!
//! Only a packaged copy (a macOS bundle or an installed Windows app) is
//! updated: a build run from `target/` has nothing to replace.

mod feed;
mod install;
mod signature;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use sha2::Digest;
use tokio::io::AsyncWriteExt;

pub use feed::{Artifact, Feed, platform, version};
pub use signature::{begin, public_key, sign};

use crate::Packaging;

/// First check, once startup has settled.
const FIRST_CHECK: Duration = Duration::from_secs(30);
/// Then this often.
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// No release is this big; a larger download is cut off.
const MAX_DOWNLOAD: u64 = 300 * 1024 * 1024;
const MAX_FEED: usize = 64 * 1024;

pub struct Config {
    /// The feed's https address.
    pub feed: String,
    /// The release key's public half, in base64.
    pub key: String,
    /// The running version.
    pub version: String,
    pub user_agent: String,
    /// Where downloads wait to be installed; emptied at startup.
    pub dir: PathBuf,
    pub packaging: Packaging,
}

/// Where updating stands, as the interface shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Checking,
    UpToDate,
    /// Downloading this version.
    Downloading(String),
    /// This version is downloaded and checked; a restart installs it.
    Ready(String),
    /// The last check failed; it is tried again later.
    Failed(String),
}

struct Staged {
    version: String,
    path: PathBuf,
}

pub struct Updater {
    checks: async_channel::Sender<()>,
    status: async_channel::Receiver<Status>,
    staged: Arc<Mutex<Option<Staged>>>,
    packaging: Packaging,
}

impl Updater {
    /// Starts the update thread.
    pub fn start(config: Config) -> Result<Self> {
        let key = public_key(&config.key).ok_or_else(|| anyhow!("bad release key"))?;
        if !config.feed.starts_with("https://") {
            bail!("the feed must be https");
        }
        let _ = std::fs::remove_dir_all(&config.dir);
        let (checks, requests) = async_channel::bounded(1);
        let (report, status) = async_channel::bounded(8);
        let staged = Arc::new(Mutex::new(None));
        let packaging = config.packaging.clone();
        let worker = Worker { config, key, report, staged: staged.clone() };
        std::thread::Builder::new().name("updates".into()).spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build();
            match runtime {
                Ok(runtime) => runtime.block_on(worker.run(requests)),
                Err(e) => tracing::warn!("updates: {e}"),
            }
        })?;
        Ok(Self { checks, status, staged, packaging })
    }

    /// Checks now instead of waiting for the next round.
    pub fn check(&self) {
        let _ = self.checks.try_send(());
    }

    /// Changes of [`Status`], as they happen.
    pub fn status(&self) -> &async_channel::Receiver<Status> {
        &self.status
    }

    /// Installs the downloaded version and arranges for it to open; the
    /// caller quits right after. Fails when nothing is ready.
    pub fn install(&self) -> Result<()> {
        let staged = self.staged.lock().map_err(|_| anyhow!("update state poisoned"))?;
        let staged = staged.as_ref().ok_or_else(|| anyhow!("no update is ready"))?;
        tracing::info!(version = staged.version, "updates: installing");
        install::install(&staged.path, &self.packaging).context("installing the update")
    }
}

struct Worker {
    config: Config,
    key: ed25519_dalek::VerifyingKey,
    report: async_channel::Sender<Status>,
    staged: Arc<Mutex<Option<Staged>>>,
}

impl Worker {
    async fn run(self, requests: async_channel::Receiver<()>) {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let http = match reqwest::Client::builder()
            .user_agent(&self.config.user_agent)
            .connect_timeout(Duration::from_secs(20))
            .build()
        {
            Ok(http) => http,
            Err(e) => return tracing::warn!("updates: {e}"),
        };
        let mut wait = FIRST_CHECK;
        loop {
            tokio::select! {
                request = requests.recv() => if request.is_err() { return },
                () = tokio::time::sleep(wait) => {}
            }
            wait = CHECK_EVERY;
            self.send(Status::Checking).await;
            let status = match self.round(&http).await {
                Ok(status) => status,
                Err(e) => {
                    tracing::warn!("updates: {e:#}");
                    Status::Failed(format!("{e:#}"))
                }
            };
            self.send(status).await;
        }
    }

    async fn send(&self, status: Status) {
        let _ = self.report.send(status).await;
    }

    /// One check: the feed, then the download when there is something new.
    async fn round(&self, http: &reqwest::Client) -> Result<Status> {
        let mut response = http.get(&self.config.feed).timeout(Duration::from_secs(30)).send().await?;
        response = response.error_for_status()?;
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if body.len() + chunk.len() > MAX_FEED {
                bail!("the feed is too large");
            }
            body.extend_from_slice(&chunk);
        }
        let feed: Feed = serde_json::from_slice(&body).context("reading the feed")?;
        let Some(artifact) = feed.newer(&self.config.version, &platform()) else {
            return Ok(Status::UpToDate);
        };
        if self.staged.lock().ok().and_then(|staged| staged.as_ref().map(|s| s.version == feed.version)) == Some(true) {
            return Ok(Status::Ready(feed.version));
        }
        tracing::info!(version = feed.version, "updates: downloading");
        self.send(Status::Downloading(feed.version.clone())).await;
        let download = self.config.dir.join(file_name(&artifact.url)?);
        self.download(http, artifact, &feed.version, &download).await?;
        let path = install::stage(&download, &self.config.packaging).context("unpacking the update")?;
        tracing::info!(version = feed.version, "updates: ready");
        if let Ok(mut staged) = self.staged.lock() {
            *staged = Some(Staged { version: feed.version.clone(), path });
        }
        Ok(Status::Ready(feed.version))
    }

    /// Downloads `artifact` to `path`, hashing as it goes; the file is
    /// removed unless its signature checks out for `version` on this
    /// platform.
    async fn download(&self, http: &reqwest::Client, artifact: &Artifact, version: &str, path: &Path) -> Result<()> {
        tokio::fs::create_dir_all(&self.config.dir).await?;
        let mut response = http.get(&artifact.url).send().await?.error_for_status()?;
        if !response.url().as_str().starts_with("https://") {
            bail!("the download left https");
        }
        let mut file = tokio::fs::File::create(path).await?;
        let mut digest = signature::begin(version, &platform());
        let mut size = 0u64;
        let written: Result<()> = async {
            while let Some(chunk) = response.chunk().await? {
                size += chunk.len() as u64;
                if size > MAX_DOWNLOAD {
                    bail!("the download is too large");
                }
                digest.update(&chunk);
                file.write_all(&chunk).await?;
            }
            file.flush().await?;
            Ok(())
        }
        .await;
        drop(file);
        let checked = written.and_then(|()| {
            if signature::verify(&self.key, digest, &artifact.signature) {
                Ok(())
            } else {
                Err(anyhow!("the download's signature does not match the release key"))
            }
        });
        if checked.is_err() {
            let _ = tokio::fs::remove_file(path).await;
        }
        checked
    }
}

/// The last part of a download's URL, kept to plain characters.
fn file_name(url: &str) -> Result<String> {
    let name = url.split(['?', '#']).next().unwrap_or(url).rsplit('/').next().unwrap_or_default();
    let valid = !name.is_empty()
        && !name.starts_with('.')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
    if valid { Ok(name.to_owned()) } else { Err(anyhow!("odd download name")) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_names_stay_in_their_folder() {
        assert_eq!(file_name("https://x/r/Emerald-0.2.0-setup.exe?a=b").unwrap(), "Emerald-0.2.0-setup.exe");
        assert!(file_name("https://x/r/..").is_err());
        assert!(file_name("https://x/r/").is_err());
        assert!(file_name("https://x/r/a%2F..%2Fb").is_err());
    }
}
