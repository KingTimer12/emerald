//! The account's own synced settings (status, custom status, privacy),
//! what the others see of it, and the devices signed in to it.

use emerald_discord::model::{AuthSession, CustomStatus, Privacy, Status, UserSettings};
use emerald_discord::{Command, OwnPresence, SettingsEdit};

#[derive(Default)]
pub struct Account {
    /// `None` until READY brings them.
    settings: Option<UserSettings>,
    /// This device among `sessions`.
    session_hash: Option<String>,
    /// The signed-in devices, once asked for: the list, or why it failed.
    sessions: Option<Result<Vec<AuthSession>, String>>,
    /// The presence last sent, so an update that changes nothing sends
    /// nothing.
    sent: Option<OwnPresence>,
    /// Picked here and on its way to Discord: shown and sent until the
    /// saved settings come back.
    picked_status: Option<Status>,
    picked_custom: Option<Option<CustomStatus>>,
    /// The login window for another account is open.
    pub adding: bool,
}

impl Account {
    pub fn load(&mut self, settings: Option<UserSettings>, session_hash: Option<String>) {
        self.settings = settings;
        self.session_hash = session_hash;
        self.sessions = None;
        // A new session starts with no presence sent.
        self.sent = None;
        self.picked_status = None;
        self.picked_custom = None;
    }

    pub fn pick_status(&mut self, status: Status) {
        self.picked_status = Some(status);
    }

    pub fn pick_custom_status(&mut self, custom: Option<CustomStatus>) {
        self.picked_custom = Some(custom);
    }

    pub fn apply(&mut self, update: UserSettings, partial: bool) {
        self.picked_status = None;
        self.picked_custom = None;
        match &mut self.settings {
            Some(settings) => settings.merge(update, partial),
            None => self.settings = Some(update),
        }
    }

    /// The status picked on any device.
    pub fn status(&self) -> Option<Status> {
        self.picked_status.or_else(|| self.settings.as_ref()?.status())
    }

    /// The custom status, unless it has run out.
    pub fn custom_status(&self, now_ms: u64) -> Option<CustomStatus> {
        let custom = match &self.picked_custom {
            Some(picked) => picked.clone(),
            None => self.settings.as_ref()?.custom_status(),
        };
        custom.filter(|c| !c.expired(now_ms))
    }

    pub fn privacy(&self) -> Option<Privacy> {
        self.settings.as_ref().map(UserSettings::privacy)
    }

    /// The command saving `edit`, made against the settings as they are;
    /// `None` before READY.
    pub fn edit(&self, edit: SettingsEdit) -> Option<Command> {
        Some(Command::EditSettings {
            current: self.settings.clone()?,
            edit,
        })
    }

    /// The presence to send now, from the synced status (else `fallback`)
    /// and custom status; `None` when it is what was last sent, or when
    /// there is nothing to say and Discord's saved status already applies.
    pub fn presence(&mut self, fallback: Option<Status>, now_ms: u64) -> Option<OwnPresence> {
        let custom = self.custom_status(now_ms);
        let status = self.status().or(fallback).or(custom.as_ref().map(|_| Status::Online))?;
        let presence = OwnPresence { status, custom };
        if self.sent.as_ref() == Some(&presence) {
            return None;
        }
        self.sent = Some(presence.clone());
        Some(presence)
    }

    /// When the custom status runs out, to clear it then.
    pub fn custom_status_expiry(&self, now_ms: u64) -> Option<u64> {
        self.custom_status(now_ms).map(|c| c.expires_at_ms).filter(|ms| *ms != 0)
    }

    pub fn sessions(&self) -> Option<&Result<Vec<AuthSession>, String>> {
        self.sessions.as_ref()
    }

    pub fn set_sessions(&mut self, sessions: Result<Vec<AuthSession>, String>) {
        self.sessions = Some(sessions);
    }

    /// Asked for again whenever the Devices page opens.
    pub fn forget_sessions(&mut self) {
        self.sessions = None;
    }

    pub fn logged_out(&mut self, hashes: &[String]) {
        if let Some(Ok(sessions)) = &mut self.sessions {
            sessions.retain(|s| !hashes.contains(&s.id_hash));
        }
    }

    /// The devices other than this one, most recently used first.
    pub fn other_sessions(&self) -> Vec<&AuthSession> {
        let Some(Ok(sessions)) = &self.sessions else { return Vec::new() };
        let mut others: Vec<_> = sessions
            .iter()
            .filter(|s| Some(s.id_hash.as_str()) != self.session_hash.as_deref())
            .collect();
        others.sort_by(|a, b| b.approx_last_used_time.cmp(&a.approx_last_used_time));
        others
    }

    pub fn this_session(&self) -> Option<&AuthSession> {
        let Some(Ok(sessions)) = &self.sessions else { return None };
        sessions.iter().find(|s| Some(s.id_hash.as_str()) == self.session_hash.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(hash: &str, used: &str) -> AuthSession {
        serde_json::from_value(serde_json::json!({ "id_hash": hash, "approx_last_used_time": used })).unwrap()
    }

    #[test]
    fn presence_goes_out_once_per_change() {
        let mut account = Account::default();
        account.load(Some(UserSettings::default()), None);
        // Nothing picked anywhere: Discord's saved status stays.
        assert_eq!(account.presence(None, 0), None);
        let sent = account.presence(Some(Status::Idle), 0).unwrap();
        assert_eq!(sent.status, Status::Idle);
        assert_eq!(account.presence(Some(Status::Idle), 0), None);
        assert!(account.presence(Some(Status::Dnd), 0).is_some());
    }

    #[test]
    fn picks_show_until_the_settings_come_back() {
        let mut account = Account::default();
        account.load(Some(UserSettings::default()), None);
        account.pick_status(Status::Invisible);
        let custom = CustomStatus {
            text: "fora".into(),
            expires_at_ms: 100,
            ..Default::default()
        };
        account.pick_custom_status(Some(custom));
        assert_eq!(account.status(), Some(Status::Invisible));
        assert_eq!(account.custom_status_expiry(50), Some(100));
        assert_eq!(account.custom_status(100), None);
        account.apply(UserSettings::default(), false);
        assert_eq!(account.status(), None);
        assert_eq!(account.custom_status(0), None);
    }

    #[test]
    fn edits_need_the_settings() {
        let mut account = Account::default();
        assert!(account.edit(SettingsEdit::FriendSources(2)).is_none());
        account.load(Some(UserSettings::default()), None);
        assert!(matches!(account.edit(SettingsEdit::FriendSources(2)), Some(Command::EditSettings { .. })));
    }

    #[test]
    fn this_device_is_kept_apart() {
        let mut account = Account::default();
        account.load(None, Some("me".into()));
        account.set_sessions(Ok(vec![
            session("old", "2024-01-01T00:00:00+00:00"),
            session("me", "2025-01-01T00:00:00+00:00"),
            session("new", "2025-06-01T00:00:00+00:00"),
        ]));
        assert_eq!(account.this_session().unwrap().id_hash, "me");
        let others: Vec<_> = account.other_sessions().iter().map(|s| s.id_hash.as_str()).collect();
        assert_eq!(others, ["new", "old"]);
        account.logged_out(&["old".into()]);
        assert_eq!(account.other_sessions().len(), 1);
    }
}
