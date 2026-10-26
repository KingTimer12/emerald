//! The account token: how it is held in memory ([`Token`]), where it is
//! stored ([`Vault`], over the [`Store`]s of each platform), and the
//! accounts signed in on this computer ([`Accounts`]).
//!
//! Release builds keep it in the OS credential store: the login Keychain on
//! macOS, Credential Manager on Windows (DPAPI, tied to the Windows user).
//! A token left in the old plain file is moved there and the file deleted.
//! Debug builds, and platforms without a store, use the file
//! (`token` under `paths::config_dir`, mode 0600): the Keychain trusts one
//! exact binary, so every rebuild would ask for the password again.

mod accounts;
#[cfg(windows)]
mod credential_manager;
mod file;
#[cfg(target_os = "macos")]
mod keychain;
mod vault;

use zeroize::Zeroizing;

pub use accounts::{Account, Accounts, MAX_ACCOUNTS, load, remove, save, switch_to, user_id};
pub use file::FileStore;
pub use vault::Vault;

/// Service and account names of the credential store item; each signed-in
/// account's item adds its id to the account name.
const SERVICE: &str = "Emerald";
const ACCOUNT: &str = "discord-token";

/// Wiped from memory when dropped and never printed.
pub struct Token(Zeroizing<String>);

impl Token {
    pub fn new(token: String) -> Option<Self> {
        let token = Zeroizing::new(token);
        let trimmed = token.trim();
        (!trimmed.is_empty()).then(|| Self(Zeroizing::new(trimmed.to_owned())))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Reads a token from bytes kept in a store, wiping the copy.
    fn from_bytes(bytes: Vec<u8>) -> std::io::Result<Option<Self>> {
        let bytes = Zeroizing::new(bytes);
        let text = String::from_utf8(bytes.to_vec()).map_err(std::io::Error::other)?;
        Ok(Self::new(text))
    }
}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Token(..)")
    }
}

/// Somewhere a token can be kept between runs.
pub trait Store {
    /// For logs: "Keychain", "token file", …
    fn name(&self) -> &'static str;
    /// `Ok(None)` when nothing is stored.
    fn read(&self) -> std::io::Result<Option<Token>>;
    fn write(&self, token: &Token) -> std::io::Result<()>;
    /// Succeeds when there was nothing to delete.
    fn delete(&self) -> std::io::Result<()>;
}

/// A token handed over in `DISCORD_TOKEN`, taken out of the environment so
/// child processes never see it. Must run while the process is still
/// single-threaded, as removing an environment variable requires.
pub fn take_env() -> Option<Token> {
    let token = std::env::var("DISCORD_TOKEN").ok().and_then(Token::new)?;
    // SAFETY: main calls this first, before any other thread starts.
    unsafe { std::env::remove_var("DISCORD_TOKEN") };
    Some(token)
}

/// Forgets the active account's token everywhere it might be.
pub fn forget() {
    accounts::forget();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_trimmed_and_never_printed() {
        assert!(Token::new("  \n".into()).is_none());
        let token = Token::new(" abc.def \n".into()).unwrap();
        assert_eq!(token.expose(), "abc.def");
        assert_eq!(format!("{token:?}"), "Token(..)");
    }
}
