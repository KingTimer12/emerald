//! The token in a plain file only the user can read.

use std::io::Write;
use std::path::PathBuf;

use super::{Store, Token};

pub struct FileStore {
    pub path: PathBuf,
}

impl Store for FileStore {
    fn name(&self) -> &'static str {
        "token file"
    }

    fn read(&self) -> std::io::Result<Option<Token>> {
        match std::fs::read(&self.path) {
            Ok(bytes) => Token::from_bytes(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Through a temporary file and a rename, so a crash never leaves half
    /// a token.
    fn write(&self, token: &Token) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("tmp");
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        // On Windows the per-user AppData ACL already keeps other users out.
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let mut file = options.open(&tmp)?;
        file.write_all(token.expose().as_bytes())?;
        file.sync_all()?;
        std::fs::rename(tmp, &self.path)
    }

    fn delete(&self) -> std::io::Result<()> {
        match std::fs::remove_file(&self.path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let dir = std::env::temp_dir().join(format!("emerald-token-file-{}", std::process::id()));
        let store = FileStore { path: dir.join("token") };
        assert!(store.read().unwrap().is_none());
        store.write(&Token::new("abc".into()).unwrap()).unwrap();
        assert_eq!(store.read().unwrap().unwrap().expose(), "abc");
        store.delete().unwrap();
        store.delete().unwrap();
        assert!(store.read().unwrap().is_none());
        std::fs::remove_dir_all(dir).ok();
    }
}
