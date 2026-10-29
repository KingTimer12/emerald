//! The accounts signed in on this computer, for switching between them.
//! The list (`accounts.json` under the config directory) holds only what
//! the switcher shows, never a token; each token sits in its account's own
//! [`Vault`]. Switching starts Emerald afresh on the other account.

use std::path::{Path, PathBuf};

use base64::Engine;
use serde::{Deserialize, Serialize};

use super::{Token, Vault};
use crate::model::{Snowflake, User};

/// How many accounts the web client lets one sign in to at once.
pub const MAX_ACCOUNTS: usize = 5;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Accounts {
    /// The one Emerald opens; the first listed when unset.
    pub active: Option<Snowflake>,
    pub list: Vec<Account>,
}

/// What the switcher shows of an account; names are filled in once it
/// connects.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: Snowflake,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub global_name: Option<String>,
    /// Avatar hash.
    #[serde(default)]
    pub avatar: Option<String>,
}

impl Account {
    pub fn display_name(&self) -> &str {
        self.global_name.as_deref().filter(|n| !n.is_empty()).unwrap_or(&self.username)
    }
}

impl Accounts {
    /// The list as saved; empty when there is none yet or it is unreadable.
    pub fn load() -> Self {
        path().map(|path| Self::read(&path)).unwrap_or_default()
    }

    fn read(path: &Path) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(path) = path() else { return };
        if let Err(e) = self.write(&path) {
            tracing::warn!("could not save the account list: {e}");
        }
    }

    fn write(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?)?;
        std::fs::rename(tmp, path)
    }

    /// The account Emerald opens.
    pub fn current(&self) -> Option<Snowflake> {
        self.active.filter(|id| self.find(*id).is_some()).or(self.list.first().map(|a| a.id))
    }

    pub fn find(&self, id: Snowflake) -> Option<&Account> {
        self.list.iter().find(|a| a.id == id)
    }

    /// Adds `id` if new and makes it the active one.
    fn activate(&mut self, id: Snowflake) {
        if self.find(id).is_none() {
            self.list.push(Account {
                id,
                username: String::new(),
                global_name: None,
                avatar: None,
            });
        }
        self.active = Some(id);
    }

    /// Takes `id` off the list; the next one becomes active when it was.
    fn remove(&mut self, id: Snowflake) {
        self.list.retain(|a| a.id != id);
        if self.active == Some(id) {
            self.active = self.list.first().map(|a| a.id);
        }
    }

    /// Keeps the switcher's name and avatar in step with the account.
    /// Returns whether anything changed.
    pub fn remember(&mut self, user: &User) -> bool {
        let Some(account) = self.list.iter_mut().find(|a| a.id == user.id) else { return false };
        let fresh = Account {
            id: user.id,
            username: user.username.clone(),
            global_name: user.global_name.clone(),
            avatar: user.avatar.clone(),
        };
        if *account == fresh {
            return false;
        }
        *account = fresh;
        true
    }
}

fn path() -> Option<PathBuf> {
    emerald_core::paths::config_dir().map(|dir| dir.join("accounts.json"))
}

/// The account a token belongs to. Its first part is the user id in
/// base64, so no request is needed.
pub fn user_id(token: &Token) -> Option<Snowflake> {
    let first = token.expose().split('.').next()?.trim_end_matches('=');
    let engines = [
        base64::engine::general_purpose::STANDARD_NO_PAD,
        base64::engine::general_purpose::URL_SAFE_NO_PAD,
    ];
    let bytes = engines.iter().find_map(|engine| engine.decode(first).ok())?;
    let text = std::str::from_utf8(&bytes).ok()?;
    text.parse().ok().map(Snowflake)
}

/// The active account's token. A token from a single-account build moves
/// into its account's vault first.
pub fn load() -> Option<Token> {
    let mut accounts = Accounts::load();
    if let Some(id) = accounts.current() {
        return Vault::account(id).load();
    }
    let legacy = Vault::default();
    let token = legacy.load()?;
    let Some(id) = user_id(&token) else {
        // Unreadable as an account: keep using it where it is.
        return Some(token);
    };
    match Vault::account(id).write(&token) {
        Ok(()) => {
            legacy.forget();
            accounts.activate(id);
            accounts.save();
            tracing::info!("moved the token into its account's slot");
        }
        Err(e) => tracing::error!("could not move the token into its account's slot: {e}"),
    }
    Some(token)
}

/// Keeps a freshly signed-in token and makes its account the active one.
pub fn save(token: &Token) -> std::io::Result<()> {
    let Some(id) = user_id(token) else {
        return Vault::default().write(token);
    };
    Vault::account(id).write(token)?;
    let mut accounts = Accounts::load();
    accounts.activate(id);
    accounts.save();
    Ok(())
}

/// Signs the active account out of this computer; the next one listed, if
/// any, becomes active.
pub fn forget() {
    let mut accounts = Accounts::load();
    match accounts.current() {
        Some(id) => {
            Vault::account(id).forget();
            accounts.remove(id);
            accounts.save();
        }
        None => Vault::default().forget(),
    }
}

/// Makes `id` the account the next start opens. `false` when it is not a
/// signed-in account.
pub fn switch_to(id: Snowflake) -> bool {
    let mut accounts = Accounts::load();
    if accounts.find(id).is_none() {
        return false;
    }
    accounts.activate(id);
    accounts.save();
    true
}

/// Signs a non-active account out of this computer.
pub fn remove(id: Snowflake) {
    let mut accounts = Accounts::load();
    if accounts.current() == Some(id) {
        return;
    }
    Vault::account(id).forget();
    accounts.remove(id);
    accounts.save();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(id: u64, name: &str) -> User {
        User {
            id: Snowflake(id),
            username: name.into(),
            global_name: None,
            avatar: None,
            email: None,
            bot: false,
            premium_type: None,
            avatar_decoration_data: None,
            collectibles: None,
        }
    }

    #[test]
    fn tokens_name_their_account() {
        // "80351110224678912" in base64, as Discord writes it.
        let token = Token::new("ODAzNTExMTAyMjQ2Nzg5MTI.Gx.abc".into()).unwrap();
        assert_eq!(user_id(&token), Some(Snowflake(80351110224678912)));
        assert_eq!(user_id(&Token::new("not-a-token".into()).unwrap()), None);
    }

    #[test]
    fn the_list_tracks_the_active_account() {
        let mut accounts = Accounts::default();
        assert_eq!(accounts.current(), None);
        accounts.activate(Snowflake(1));
        accounts.activate(Snowflake(2));
        assert_eq!(accounts.current(), Some(Snowflake(2)));
        assert!(accounts.remember(&user(2, "bia")));
        assert!(!accounts.remember(&user(2, "bia")));
        assert_eq!(accounts.find(Snowflake(2)).unwrap().display_name(), "bia");
        accounts.remove(Snowflake(2));
        assert_eq!(accounts.current(), Some(Snowflake(1)));
        assert_eq!(accounts.list.len(), 1);
    }

    #[test]
    fn the_list_is_saved_without_tokens() {
        let dir = std::env::temp_dir().join(format!("emerald-accounts-{}", std::process::id()));
        let path = dir.join("accounts.json");
        let mut accounts = Accounts::default();
        accounts.activate(Snowflake(7));
        accounts.write(&path).unwrap();
        assert_eq!(Accounts::read(&path), accounts);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"7\"") && !text.contains("token"));
        std::fs::remove_dir_all(dir).ok();
    }
}
