//! Whole-URL downloads (CDN images, media proxies, GIF providers), kept in
//! the disk [`Cache`] so a restart does not fetch them again.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, bail};

use crate::cache::Cache;
use crate::error::Local;

/// A plain client: no API headers and no credentials, as a browser loads
/// an `<img>`.
pub struct Downloads {
    http: reqwest::Client,
    /// `None` runs uncached.
    cache: Option<Arc<Cache>>,
}

impl Downloads {
    pub fn new(user_agent: &'static str, cache: Option<Arc<Cache>>) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(user_agent)
            .pool_max_idle_per_host(2)
            .timeout(Duration::from_secs(20))
            .build()?;
        Ok(Self { http, cache })
    }

    /// `url` from the disk cache, or downloaded and cached. Refuses
    /// anything over `max_bytes`.
    pub async fn get(&self, url: &str, max_bytes: usize) -> Result<Vec<u8>> {
        if let Some(bytes) = on_disk(&self.cache, |cache, url| cache.image(url), url).await.flatten() {
            return Ok(bytes);
        }
        let bytes = self.download(url, max_bytes).await?;
        let stored = bytes.clone();
        on_disk(&self.cache, move |cache, url| cache.put_image(url, &stored), url).await;
        Ok(bytes)
    }

    async fn download(&self, url: &str, max_bytes: usize) -> Result<Vec<u8>> {
        let response = self.http.get(url).send().await?.error_for_status()?;
        if response.content_length().is_some_and(|len| len as usize > max_bytes) {
            bail!(Local::ImageTooLarge);
        }
        let bytes = response.bytes().await?;
        if bytes.len() > max_bytes {
            bail!(Local::ImageTooLarge);
        }
        Ok(bytes.to_vec())
    }
}

/// Runs `op` on the disk cache off the async thread; `None` when there is
/// no cache.
pub(crate) async fn on_disk<T: Send + 'static>(
    cache: &Option<Arc<Cache>>,
    op: impl FnOnce(&Cache, &str) -> T + Send + 'static,
    key: &str,
) -> Option<T> {
    let cache = cache.clone()?;
    let key = key.to_owned();
    tokio::task::spawn_blocking(move || op(&cache, &key)).await.ok()
}
