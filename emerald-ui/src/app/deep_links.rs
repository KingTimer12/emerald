//! Links the app is opened with: `discord://` links as the web client makes
//! them (`discord://discord.com/channels/…`; `discord://-/…` and
//! `discord:///…` too), and Discord's own https links. A channel link opens
//! the channel; an invite only fills the "Join a Server" form, so joining
//! stays the user's click.

use super::*;
use crate::markdown::{channel_link, invite_link};

#[derive(Debug, PartialEq, Eq)]
pub(super) enum DeepLink {
    Channel { guild: Option<Snowflake>, channel: Snowflake, message: Option<Snowflake> },
    Invite(String),
}

impl DeepLink {
    pub(super) fn parse(url: &str) -> Option<Self> {
        let url = url.trim();
        // The host (`discord.com`, `-` or none) does not matter; the path does.
        let https = match url.strip_prefix("discord://") {
            Some(rest) => format!("https://discord.com/{}", rest.split_once('/').map_or("", |(_, path)| path)),
            None => url.to_owned(),
        };
        if let Some((guild, channel, message)) = channel_link(&https) {
            return Some(Self::Channel { guild, channel, message });
        }
        invite_link(&https).map(|code| Self::Invite(code.to_owned()))
    }
}

impl Emerald {
    /// Opens a link the app was launched with, or was handed while running.
    /// Before the first READY there is nothing to open it in, so it waits.
    pub fn open_deep_link(&mut self, url: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(link) = DeepLink::parse(url) else {
            tracing::info!("ignored a link the app was opened with");
            return;
        };
        window.activate_window();
        if !self.connection.connected && self.spaces.len() <= 1 {
            self.pending_link = Some(link);
            return;
        }
        self.follow_deep_link(link, window, cx);
    }

    /// The link that waited for READY.
    pub(super) fn follow_pending_link(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(link) = self.pending_link.take() {
            self.follow_deep_link(link, window, cx);
        }
    }

    fn follow_deep_link(&mut self, link: DeepLink, window: &mut Window, cx: &mut Context<Self>) {
        match link {
            DeepLink::Channel { guild, channel, message } => {
                self.close_overlay(window, cx);
                self.open_channel_link(guild, channel, message, window, cx);
            }
            DeepLink::Invite(code) => self.open_join_server(&code, window, cx),
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_links_open_channels_and_invites() {
        let channel = |guild, channel, message| Some(DeepLink::Channel { guild, channel, message });
        assert_eq!(
            DeepLink::parse("discord://discord.com/channels/@me/12"),
            channel(None, Snowflake(12), None)
        );
        assert_eq!(
            DeepLink::parse("discord://-/channels/1/2/3"),
            channel(Some(Snowflake(1)), Snowflake(2), Some(Snowflake(3)))
        );
        assert_eq!(DeepLink::parse("discord:///invite/abc-D"), Some(DeepLink::Invite("abc-D".into())));
        assert_eq!(DeepLink::parse("https://discord.gg/abc"), Some(DeepLink::Invite("abc".into())));
        assert_eq!(DeepLink::parse("discord://app/settings/nitro"), None);
        assert_eq!(DeepLink::parse("--login"), None);
    }
}
