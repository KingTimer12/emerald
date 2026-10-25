//! Where the token lives on this build and platform: the OS credential
//! store in release builds that have one, the file otherwise, and the move
//! from the file into the store.

use super::{FileStore, SERVICE, Store, Token};

pub struct Vault {
    /// The OS credential store; `None` keeps the token in `file`.
    secure: Option<Box<dyn Store>>,
    /// Where debug builds keep it, and where older builds left it.
    file: Option<FileStore>,
}

impl Default for Vault {
    fn default() -> Self {
        let secure = if cfg!(debug_assertions) { None } else { secure_store(SERVICE) };
        let file = emerald_core::paths::config_dir().map(|dir| FileStore { path: dir.join("token") });
        Self { secure, file }
    }
}

/// The platform's credential store under `service`, if it has one.
#[cfg(target_os = "macos")]
fn secure_store(service: &'static str) -> Option<Box<dyn Store>> {
    Some(Box::new(super::keychain::Keychain { service }))
}

#[cfg(windows)]
fn secure_store(service: &'static str) -> Option<Box<dyn Store>> {
    Some(Box::new(super::credential_manager::CredentialManager { service }))
}

#[cfg(not(any(target_os = "macos", windows)))]
fn secure_store(_service: &'static str) -> Option<Box<dyn Store>> {
    None
}

impl Vault {
    /// The stored token. One found only in the old file moves into the
    /// credential store.
    pub fn load(&self) -> Option<Token> {
        let Some(secure) = &self.secure else {
            let token = self.read_file();
            log_loaded(token.is_some(), "token file");
            return token;
        };
        match secure.read() {
            Ok(Some(token)) => {
                log_loaded(true, secure.name());
                return Some(token);
            }
            Ok(None) => {}
            Err(e) => tracing::error!("could not read the token from the {}: {e}", secure.name()),
        }
        let token = self.read_file();
        log_loaded(token.is_some(), "token file");
        let token = token?;
        match secure.write(&token) {
            Ok(()) => {
                tracing::info!("moved the token from its file into the {}", secure.name());
                self.delete_file();
            }
            Err(e) => tracing::error!("could not move the token to the {}: {e}", secure.name()),
        }
        Some(token)
    }

    pub fn write(&self, token: &Token) -> std::io::Result<()> {
        match (&self.secure, &self.file) {
            (Some(secure), _) => secure.write(token),
            (None, Some(file)) => file.write(token),
            (None, None) => Err(std::io::ErrorKind::NotFound.into()),
        }
    }

    /// Deletes the token from the store and the file alike.
    pub fn forget(&self) {
        if let Some(secure) = &self.secure
            && let Err(e) = secure.delete()
        {
            tracing::error!("could not delete the token from the {}: {e}", secure.name());
        }
        self.delete_file();
    }

    fn read_file(&self) -> Option<Token> {
        self.file.as_ref()?.read().ok().flatten()
    }

    fn delete_file(&self) {
        let Some(file) = &self.file else { return };
        if let Err(e) = file.delete() {
            tracing::error!("could not delete {}: {e}", file.path.display());
        }
    }
}

fn log_loaded(found: bool, from: &str) {
    if found {
        tracing::info!("token loaded from the {from}");
    } else {
        tracing::info!("no stored token");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Under a test-only service name and a temporary file, so the user's
    /// token is never touched.
    #[cfg(any(target_os = "macos", windows))]
    #[test]
    fn credential_store_round_trip_and_migration() {
        let dir = std::env::temp_dir().join(format!("emerald-token-vault-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("token");
        let vault = Vault {
            secure: secure_store("Emerald (test)"),
            file: Some(FileStore { path: path.clone() }),
        };
        vault.forget();
        assert!(vault.load().is_none());
        vault.write(&Token::new("first".into()).unwrap()).unwrap();
        vault.write(&Token::new("second".into()).unwrap()).unwrap();
        assert_eq!(vault.load().unwrap().expose(), "second");
        vault.forget();
        assert!(vault.load().is_none());

        // A plain file from an older build moves into the store and is deleted.
        std::fs::write(&path, "from-file\n").unwrap();
        assert_eq!(vault.load().unwrap().expose(), "from-file");
        assert!(!path.exists());
        assert_eq!(vault.secure.as_ref().unwrap().read().unwrap().unwrap().expose(), "from-file");
        vault.forget();
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn without_a_store_the_file_is_used() {
        let dir = std::env::temp_dir().join(format!("emerald-token-plain-{}", std::process::id()));
        let vault = Vault {
            secure: None,
            file: Some(FileStore { path: dir.join("token") }),
        };
        vault.write(&Token::new("plain".into()).unwrap()).unwrap();
        assert_eq!(vault.load().unwrap().expose(), "plain");
        vault.forget();
        assert!(vault.load().is_none());
        std::fs::remove_dir_all(dir).ok();
    }
}
