//! The release key and the update feed, for whoever publishes releases.
//!
//! ```sh
//! # Once: a new key. The secret goes to FILE (keep it out of the
//! # repository); the public half is printed, for the app to embed.
//! cargo run -p emerald-desktop --example release_key -- keygen FILE
//!
//! # Every release, with the secret in EMERALD_UPDATE_KEY (base64):
//! cargo run -p emerald-desktop --example release_key -- feed VERSION BASE_URL \
//!     macos-aarch64=dist/Emerald-VERSION-macos-aarch64.app.tar.gz \
//!     windows-x86_64=dist/Emerald-VERSION-setup-x86_64.exe > latest.json
//! ```

use std::collections::BTreeMap;
use std::io::Read;

use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use ed25519_dalek::SigningKey;
use emerald_desktop::update::{Artifact, Feed, sign};
use sha2::{Digest, Sha512};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("keygen") => keygen(args.get(1).ok_or_else(|| anyhow!("keygen FILE"))?),
        Some("feed") if args.len() >= 4 => feed(&args[1], &args[2], &args[3..]),
        _ => bail!("usage: keygen FILE | feed VERSION BASE_URL PLATFORM=FILE…"),
    }
}

fn keygen(path: &str) -> Result<()> {
    if std::path::Path::new(path).exists() {
        bail!("{path} exists; not overwriting a key");
    }
    let mut seed = [0u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut random| random.read_exact(&mut seed))
        .context("reading /dev/urandom (run this on macOS or Linux)")?;
    let key = SigningKey::from_bytes(&seed);
    write_secret(path, &STANDARD.encode(seed))?;
    println!("{}", STANDARD.encode(key.verifying_key().to_bytes()));
    Ok(())
}

#[cfg(unix)]
fn write_secret(path: &str, secret: &str) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path)?;
    Ok(file.write_all(secret.as_bytes())?)
}

#[cfg(not(unix))]
fn write_secret(_: &str, _: &str) -> Result<()> {
    bail!("make the key on macOS or Linux")
}

fn feed(version: &str, base_url: &str, files: &[String]) -> Result<()> {
    let secret = std::env::var("EMERALD_UPDATE_KEY").context("EMERALD_UPDATE_KEY is not set")?;
    let seed: [u8; 32] = STANDARD
        .decode(secret.trim())
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| anyhow!("EMERALD_UPDATE_KEY is not a base64 key"))?;
    let key = SigningKey::from_bytes(&seed);
    let mut platforms = BTreeMap::new();
    for entry in files {
        let (platform, path) = entry.split_once('=').ok_or_else(|| anyhow!("{entry}: expected PLATFORM=FILE"))?;
        let mut digest = Sha512::new();
        std::io::copy(&mut std::fs::File::open(path).with_context(|| path.to_owned())?, &mut digest)?;
        let name = std::path::Path::new(path).file_name().ok_or_else(|| anyhow!("{path}: no file name"))?;
        platforms.insert(
            platform.to_owned(),
            Artifact {
                url: format!("{}/{}", base_url.trim_end_matches('/'), name.to_string_lossy()),
                signature: sign(&key, digest),
            },
        );
    }
    let feed = Feed { version: version.trim_start_matches('v').to_owned(), notes: String::new(), platforms };
    println!("{}", serde_json::to_string_pretty(&feed)?);
    Ok(())
}
