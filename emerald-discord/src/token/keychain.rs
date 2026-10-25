//! The token in the macOS login Keychain.

use security_framework::passwords::{delete_generic_password, get_generic_password, set_generic_password};

use super::{ACCOUNT, Store, Token};

/// errSecItemNotFound.
const NOT_FOUND: i32 = -25300;

/// A generic password item under `service`.
pub struct Keychain {
    pub service: &'static str,
}

impl Store for Keychain {
    fn name(&self) -> &'static str {
        "Keychain"
    }

    fn read(&self) -> std::io::Result<Option<Token>> {
        match get_generic_password(self.service, ACCOUNT) {
            Ok(bytes) => Token::from_bytes(bytes),
            Err(e) if e.code() == NOT_FOUND => Ok(None),
            Err(e) => Err(std::io::Error::other(e)),
        }
    }

    fn write(&self, token: &Token) -> std::io::Result<()> {
        set_generic_password(self.service, ACCOUNT, token.expose().as_bytes()).map_err(std::io::Error::other)
    }

    fn delete(&self) -> std::io::Result<()> {
        match delete_generic_password(self.service, ACCOUNT) {
            Err(e) if e.code() != NOT_FOUND => Err(std::io::Error::other(e)),
            _ => Ok(()),
        }
    }
}
