//! On-disk cache in one SQLite database (`cache.sqlite` under
//! `paths::cache_dir`), standing in for the browser's HTTP cache.
//!
//! - `images`: CDN downloads. Avatar and sticker URLs embed a content hash or
//!   an immutable id, so an entry never goes stale; the table only needs a
//!   size bound, enforced least recently used first.
//! - `responses`: API answers a route asked to keep (see `rest::Policy`),
//!   per account, so one that fails can fall back to the last good copy.
//!   Cleared on sign-out.
//!
//! Blocking; async callers go through `spawn_blocking`.

use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use rusqlite::{Connection, OptionalExtension, params};

/// Images are pruned back to `PRUNE_TO` once they grow past this.
const MAX_BYTES: i64 = 200 * 1024 * 1024;
const PRUNE_TO: i64 = 150 * 1024 * 1024;
/// Kept responses older than this are deleted.
const RESPONSE_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 3600);
/// A read refreshes an image's `used` time at most this often, so viewing
/// cached avatars does not turn into a stream of writes.
const TOUCH_EVERY: i64 = 3600;
const SCHEMA_VERSION: i64 = 1;

pub struct Cache {
    db: Mutex<Connection>,
}

impl Cache {
    /// The cache under `paths::cache_dir`. `None` when it cannot be opened;
    /// the app then simply runs uncached.
    pub fn open_default() -> Option<Self> {
        let dir = emerald_core::paths::cache_dir()?;
        // The file-per-image cache of earlier builds.
        let legacy = dir.join("cdn");
        if legacy.is_dir() {
            std::fs::remove_dir_all(&legacy).ok();
        }
        if let Err(e) = create_private_dir(&dir) {
            tracing::warn!("cache disabled: {e}");
            return None;
        }
        match Self::open(&dir.join("cache.sqlite")) {
            Ok(cache) => Some(cache),
            Err(e) => {
                tracing::warn!("cache disabled: {e}");
                None
            }
        }
    }

    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let cache = Self::setup(Connection::open(path)?)?;
        for suffix in ["", "-wal", "-shm"] {
            let mut file = path.as_os_str().to_owned();
            file.push(suffix);
            restrict(Path::new(&file));
        }
        Ok(cache)
    }

    #[cfg(test)]
    pub fn in_memory() -> Self {
        Self::setup(Connection::open_in_memory().unwrap()).unwrap()
    }

    fn setup(db: Connection) -> rusqlite::Result<Self> {
        db.busy_timeout(Duration::from_secs(2))?;
        // auto_vacuum only takes effect before the first table exists.
        db.execute_batch(
            "PRAGMA auto_vacuum = INCREMENTAL;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;",
        )?;
        let version: i64 = db.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version != SCHEMA_VERSION {
            // A cache: on any other layout, start over.
            db.execute_batch(&format!(
                "DROP TABLE IF EXISTS images;
                 DROP TABLE IF EXISTS responses;
                 CREATE TABLE images (
                     key TEXT PRIMARY KEY,
                     bytes BLOB NOT NULL,
                     used INTEGER NOT NULL
                 );
                 CREATE INDEX images_used ON images (used);
                 CREATE TABLE responses (
                     account INTEGER NOT NULL,
                     key TEXT NOT NULL,
                     body BLOB NOT NULL,
                     fetched INTEGER NOT NULL,
                     PRIMARY KEY (account, key)
                 );
                 PRAGMA user_version = {SCHEMA_VERSION};"
            ))?;
        }
        Ok(Self { db: Mutex::new(db) })
    }

    pub fn image(&self, url: &str) -> Option<Vec<u8>> {
        let db = self.db.lock().unwrap();
        let key = image_key(url);
        let bytes = db
            .query_row("SELECT bytes FROM images WHERE key = ?1", [&key], |row| row.get(0))
            .optional()
            .unwrap_or_else(|e| {
                tracing::warn!("image cache read: {e}");
                None
            })?;
        let now = unix_now();
        db.execute(
            "UPDATE images SET used = ?1 WHERE key = ?2 AND used < ?1 - ?3",
            params![now, key, TOUCH_EVERY],
        )
        .ok();
        Some(bytes)
    }

    pub fn put_image(&self, url: &str, bytes: &[u8]) {
        let db = self.db.lock().unwrap();
        if let Err(e) = db.execute(
            "INSERT OR REPLACE INTO images (key, bytes, used) VALUES (?1, ?2, ?3)",
            params![image_key(url), bytes, unix_now()],
        ) {
            tracing::warn!("image cache write: {e}");
        }
    }

    /// The kept answer for `key` and when it was fetched.
    pub fn response(&self, account: u64, key: &str) -> Option<(Vec<u8>, SystemTime)> {
        let db = self.db.lock().unwrap();
        let (body, fetched): (Vec<u8>, i64) = db
            .query_row(
                "SELECT body, fetched FROM responses WHERE account = ?1 AND key = ?2",
                params![account as i64, key],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .ok()??;
        Some((body, SystemTime::UNIX_EPOCH + Duration::from_secs(fetched.max(0) as u64)))
    }

    pub fn put_response(&self, account: u64, key: &str, body: &[u8]) {
        let db = self.db.lock().unwrap();
        if let Err(e) = db.execute(
            "INSERT OR REPLACE INTO responses (account, key, body, fetched) VALUES (?1, ?2, ?3, ?4)",
            params![account as i64, key, body, unix_now()],
        ) {
            tracing::warn!("response cache write: {e}");
        }
    }

    /// Drops every kept response, for every account.
    pub fn clear_responses(&self) {
        let db = self.db.lock().unwrap();
        if let Err(e) = db.execute("DELETE FROM responses", []) {
            tracing::warn!("response cache clear: {e}");
        }
    }

    /// Keeps the images under budget, most recently used first, drops old
    /// responses and gives the freed pages back to the file system.
    pub fn prune(&self) {
        let db = self.db.lock().unwrap();
        let result = db
            .execute(
                "DELETE FROM images WHERE key IN (
                     SELECT key FROM (
                         SELECT key, SUM(length(bytes)) OVER (ORDER BY used DESC, key) AS total
                         FROM images
                     ) WHERE total > ?1
                 ) AND (SELECT SUM(length(bytes)) FROM images) > ?2",
                params![PRUNE_TO, MAX_BYTES],
            )
            .and_then(|_| {
                let oldest = unix_now() - RESPONSE_MAX_AGE.as_secs() as i64;
                db.execute("DELETE FROM responses WHERE fetched < ?1", [oldest])
            })
            .and_then(|_| db.execute_batch("PRAGMA incremental_vacuum;"));
        if let Err(e) = result {
            tracing::warn!("cache prune: {e}");
        }
    }
}

/// Key for `url`. Attachment URLs carry a signature (`ex`, `is`, `hm`) that
/// changes on every fetch of the message; it is left out so one image keeps
/// one entry.
fn image_key(url: &str) -> String {
    let (base, query) = url.split_once('?').unwrap_or((url, ""));
    let kept: Vec<&str> = query
        .split('&')
        .filter(|param| !param.is_empty())
        .filter(|param| !["ex=", "is=", "hm="].iter().any(|signed| param.starts_with(signed)))
        .collect();
    if kept.is_empty() {
        base.to_owned()
    } else {
        format!("{base}?{}", kept.join("&"))
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// The cache holds private data (profiles, images from DMs): only the user
/// may read it. On Windows the per-user LocalAppData ACL already does that.
fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Owner-only, for the database and its `-wal` and `-shm` files.
fn restrict(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).ok();
    }
    #[cfg(not(unix))]
    let _ = path;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_keys_ignore_attachment_signatures() {
        assert_eq!(
            image_key("https://m/a/1.png?ex=1&is=2&hm=3&width=400"),
            image_key("https://m/a/1.png?ex=9&is=8&hm=7&width=400")
        );
        assert_eq!(image_key("https://m/a/1.png?ex=1&is=2&hm=3"), "https://m/a/1.png");
        assert_ne!(image_key("https://a/1.png?size=64"), image_key("https://a/1.png?size=128"));
    }

    #[test]
    fn images_round_trip() {
        let cache = Cache::in_memory();
        assert_eq!(cache.image("https://a/1.png"), None);
        cache.put_image("https://a/1.png?ex=1", b"png");
        assert_eq!(cache.image("https://a/1.png?ex=2").as_deref(), Some(&b"png"[..]));
    }

    #[test]
    fn prune_keeps_the_most_recently_used() {
        let cache = Cache::in_memory();
        let chunk = vec![0u8; 40 * 1024 * 1024];
        {
            let db = cache.db.lock().unwrap();
            for (i, used) in [5, 1, 4, 2, 3, 6].into_iter().enumerate() {
                db.execute(
                    "INSERT INTO images (key, bytes, used) VALUES (?1, ?2, ?3)",
                    params![format!("k{i}"), chunk, used],
                )
                .unwrap();
            }
        }
        // 240 MB > 200 MB: back under 150 MB, keeping uses 6, 5 and 4.
        cache.prune();
        let db = cache.db.lock().unwrap();
        let mut kept: Vec<i64> = db
            .prepare("SELECT used FROM images ORDER BY used")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        kept.sort();
        assert_eq!(kept, [4, 5, 6]);
    }

    #[test]
    fn prune_leaves_a_cache_under_budget_alone() {
        let cache = Cache::in_memory();
        cache.put_image("a", &[0; 10]);
        cache.prune();
        assert!(cache.image("a").is_some());
    }

    #[test]
    fn responses_are_per_account_and_cleared() {
        let cache = Cache::in_memory();
        cache.put_response(1, "/users/2/profile", b"{}");
        assert!(cache.response(1, "/users/2/profile").is_some());
        assert!(cache.response(9, "/users/2/profile").is_none());
        cache.clear_responses();
        assert!(cache.response(1, "/users/2/profile").is_none());
    }

    #[test]
    fn reopening_keeps_entries() {
        let dir = std::env::temp_dir().join(format!("emerald-cache-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cache.sqlite");
        Cache::open(&path).unwrap().put_image("x", b"1");
        assert_eq!(Cache::open(&path).unwrap().image("x").as_deref(), Some(&b"1"[..]));
        std::fs::remove_dir_all(&dir).ok();
    }
}
