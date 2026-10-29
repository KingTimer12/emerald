//! The account's own state on screen: the presence sent to the others, the
//! status and custom status, privacy choices, signed-in devices and the
//! accounts signed in on this computer.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gpui::{AppContext, Context};

use super::Emerald;
use emerald_discord::model::{CustomStatus, Snowflake, Status, User, UserProfileDetails, UserSettings};
use emerald_discord::token::Accounts;
use emerald_discord::{Action, Command, SettingsEdit};

pub(super) fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

impl Emerald {
    pub(super) fn load_account(&mut self, settings: Option<UserSettings>, session_hash: Option<String>, cx: &mut Context<Self>) {
        self.account.load(settings, session_hash);
        if let Some(status) = self.account.status() {
            self.settings.status = Some(status);
        }
        self.send_presence(cx);
        if let Some(me) = self.people.me().cloned() {
            remember_account(&me);
        }
    }

    #[hotpath::measure]
    pub(super) fn on_user_settings(&mut self, settings: UserSettings, partial: bool, cx: &mut Context<Self>) {
        self.account.apply(settings, partial);
        // Another device may have picked a status.
        if let Some(status) = self.account.status()
            && self.settings.status != Some(status)
        {
            self.settings.status = Some(status);
            self.save_settings(cx);
        }
        self.send_presence(cx);
    }

    /// Tells the gateway what the others should see, if that changed, and
    /// clears the custom status from it when it runs out.
    pub(super) fn send_presence(&mut self, cx: &mut Context<Self>) {
        let now = now_ms();
        if let Some(presence) = self.account.presence(self.settings.status, now) {
            self.net.post(Command::SetPresence(presence));
        }
        if let Some(expiry) = self.account.custom_status_expiry(now) {
            let wait = Duration::from_millis(expiry.saturating_sub(now) + 500);
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(wait).await;
                this.update(cx, |this, cx| {
                    this.send_presence(cx);
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
    }

    pub(super) fn set_status(&mut self, status: Status, cx: &mut Context<Self>) {
        self.settings.status = Some(status);
        self.account.pick_status(status);
        if let Some(command) = self.account.edit(SettingsEdit::Status(status)) {
            self.net.post(command);
        }
        self.send_presence(cx);
        self.save_settings(cx);
    }

    /// `None` clears it.
    pub(super) fn set_custom_status(&mut self, custom: Option<CustomStatus>, cx: &mut Context<Self>) {
        let custom = custom.filter(|c| !c.is_empty());
        let Some(command) = self.account.edit(SettingsEdit::CustomStatus(custom.clone())) else { return };
        if self.run(Action::AccountSettings, command, cx) {
            self.account.pick_custom_status(custom);
            self.send_presence(cx);
        }
    }

    /// A privacy change, saved as one settings edit.
    pub(super) fn edit_privacy(&mut self, edit: SettingsEdit, cx: &mut Context<Self>) {
        if let Some(command) = self.account.edit(edit) {
            self.run(Action::AccountSettings, command, cx);
        }
    }

    /// The servers joined, to apply a privacy default to them all.
    pub(super) fn joined_guilds(&self) -> Vec<Snowflake> {
        self.spaces.iter().filter(|s| !s.is_dms()).map(|s| s.id).collect()
    }

    pub(super) fn on_profile_edited(&mut self, guild: Option<Snowflake>, details: UserProfileDetails) {
        let me = self.people.my_id();
        if let Some(profile) = self.profiles.get_mut(&(me, guild)) {
            match guild {
                Some(_) => profile.guild_member_profile = Some(details),
                None => {
                    profile.user.accent_color = details.accent_color;
                    profile.user.banner = details.banner.clone();
                    profile.user_profile = details;
                }
            }
        }
    }

    /// The devices list, asked for each time the Devices page opens.
    pub(super) fn fetch_sessions(&mut self) {
        self.account.forget_sessions();
        self.net.post(Command::Sessions);
    }

    pub(super) fn log_out_sessions(&mut self, hashes: Vec<String>, cx: &mut Context<Self>) {
        if !hashes.is_empty() {
            self.run(Action::Sessions, Command::LogOutSessions(hashes), cx);
        }
    }

    /// Opens the login window for another account in the background; once
    /// signed in, Emerald starts again on it.
    pub(super) fn add_account(&mut self, cx: &mut Context<Self>) {
        if std::mem::replace(&mut self.account.adding, true) {
            return;
        }
        cx.notify();
        let login = cx.background_spawn(async { emerald_discord::add_account() });
        cx.spawn(async move |this, cx| {
            if login.await {
                emerald_discord::restart();
            }
            this.update(cx, |this, cx| {
                this.account.adding = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

/// Keeps the account switcher's name and avatar for `me` current.
pub(super) fn remember_account(me: &User) {
    let mut accounts = Accounts::load();
    if accounts.remember(me) {
        accounts.save();
    }
}
