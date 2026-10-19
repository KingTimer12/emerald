//! On-disk cache for CDN images, standing in for the browser's HTTP cache.
//! Avatar and sticker URLs embed a content hash or an immutable id, so a
//! cached file never goes stale; the cache only needs a size bound.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Pruned back to `PRUNE_TO` once it grows past this.
const MAX_BYTES: u64 = 200 * 1024 * 1024;
const PRUNE_TO: u64 = 150 * 1024 * 1024;

/// `~/Library/Caches/emerald/cdn` on macOS, `$XDG_CACHE_HOME/emerald/cdn`
/// (or `~/.cache/emerald/cdn`) elsewhere.
pub fn dir() -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let base = if cfg!(target_os = "macos") {
        home.join("Library/Caches")
    } else {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".cache"))
    };
    Some(base.join("emerald/cdn"))
}

/// File name for `url`: 64-bit FNV-1a, stable across builds and platforms.
/// Attachment URLs carry a signature (`ex`, `is`, `hm`) that changes on
/// every fetch of the message; it is left out so one image keeps one entry.
fn key(url: &str) -> String {
    let (base, query) = url.split_once('?').unwrap_or((url, ""));
    let kept = query
        .split('&')
        .filter(|param| !["ex=", "is=", "hm="].iter().any(|signed| param.starts_with(signed)));
    let hash = std::iter::once(base)
        .chain(kept)
        .flat_map(|part| part.bytes().chain(std::iter::once(b'&')))
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ byte as u64).wrapping_mul(0x0100_0000_01b3)
        });
    format!("{hash:016x}")
}

pub async fn get(dir: &Path, url: &str) -> Option<Vec<u8>> {
    tokio::fs::read(dir.join(key(url))).await.ok()
}

/// Writes through a temporary file so a crash never leaves a torn image.
pub async fn put(dir: &Path, url: &str, bytes: &[u8]) {
    let path = dir.join(key(url));
    let tmp = path.with_extension("tmp");
    let result = async {
        tokio::fs::create_dir_all(dir).await?;
        tokio::fs::write(&tmp, bytes).await?;
        tokio::fs::rename(&tmp, &path).await
    }
    .await;
    if let Err(e) = result {
        log::warn!("image cache write: {e}");
    }
}

/// Deletes the least recently written files while the cache is over budget.
/// Blocking; run it off the async runtime.
pub fn prune(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut files: Vec<(SystemTime, u64, PathBuf)> = entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let meta = entry.metadata().ok()?;
            meta.is_file()
                .then(|| (meta.modified().unwrap_or(SystemTime::UNIX_EPOCH), meta.len(), entry.path()))
        })
        .collect();
    let mut total: u64 = files.iter().map(|(_, len, _)| len).sum();
    if total <= MAX_BYTES {
        return;
    }
    files.sort_unstable_by_key(|(modified, _, _)| *modified);
    for (_, len, path) in files {
        if total <= PRUNE_TO {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total -= len;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_stable_and_distinct() {
        assert_eq!(key("https://a/1.png"), key("https://a/1.png"));
        assert_ne!(key("https://a/1.png"), key("https://a/2.png"));
        assert_ne!(key("https://a/1.png?size=64"), key("https://a/1.png?size=128"));
    }

    #[test]
    fn keys_ignore_attachment_signatures() {
        assert_eq!(
            key("https://m/a/1.png?ex=1&is=2&hm=3&width=400"),
            key("https://m/a/1.png?ex=9&is=8&hm=7&width=400")
        );
    }
}
