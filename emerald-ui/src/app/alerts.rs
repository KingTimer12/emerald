//! Desktop notifications for new messages and the unread count on the app
//! icon. Whether a message notifies follows the user's settings
//! ([`NotifySettings`](super::state::NotifySettings)); showing it is left to
//! an [`Alerts`] the executable provides.

use gpui::{Context, Window};

use super::Emerald;
use super::state::notify::{Ping, Place};
use emerald_discord::model::{Message, Snowflake, Status};

/// Longest notification body; Windows and macOS cut longer ones anyway.
const MAX_BODY: usize = 200;

/// Where notifications and the badge go.
pub trait Alerts {
    /// Shows a notification; clicking it should open `channel`.
    fn notify(&self, title: &str, body: &str, channel: Snowflake);
    /// The number on the app's icon; zero hides it.
    fn badge(&self, count: u32);
}

/// No notifications, for tests and platforms without them.
impl Alerts for () {
    fn notify(&self, _: &str, _: &str, _: Snowflake) {}
    fn badge(&self, _: u32) {}
}

impl Emerald {
    /// A message from someone else arrived: notifies when the user's
    /// settings ask for it and they are not already looking at it.
    pub(super) fn alert_message(&mut self, message: &Message, window_active: bool) {
        let channel = message.channel_id;
        let looking = window_active && self.selected_channel == Some(channel) && !self.friends_open;
        if looking || self.settings.status == Some(Status::Dnd) {
            return;
        }
        let place = self.place_of(message.guild_id, channel);
        let me = self.people.my_id();
        let my_roles = message
            .guild_id
            .and_then(|g| self.spaces.guild(g))
            .map_or(&[][..], |g| g.my_roles());
        let ping = Ping {
            user: message.mentions.iter().any(|u| u.id == me),
            everyone: message.mention_everyone,
            role: message.mention_roles.iter().any(|r| my_roles.contains(r)),
            flags: message.flags,
        };
        if !self.notify.should_notify(&place, ping, crate::time::now_secs()) {
            return;
        }
        let author = message
            .member
            .as_ref()
            .and_then(|m| m.nick.as_deref())
            .unwrap_or_else(|| message.author.display_name());
        let title = match (message.guild_id, self.spaces.channel(channel)) {
            (Some(guild), Some(found)) => {
                let server = self.spaces.iter().find(|s| s.id == guild).map(|s| s.name.to_string()).unwrap_or_default();
                format!("{author} (#{}, {server})", found.name.as_deref().unwrap_or("?"))
            }
            _ => author.to_owned(),
        };
        let body = self.notification_body(message);
        self.alerts.notify(&title, &body, channel);
    }

    /// The server, channel and the levels above it, for the settings.
    pub(super) fn place_of(&self, guild: Option<Snowflake>, channel: Snowflake) -> Place {
        let mut chain = vec![channel];
        let mut at = channel;
        // A thread's channel, then that channel's category.
        while chain.len() < 3
            && let Some(parent) = self.spaces.channel(at).and_then(|c| c.parent_id)
        {
            chain.push(parent);
            at = parent;
        }
        Place { guild, chain }
    }

    /// The message as one line of plain text: mentions by name, custom
    /// emoji by their name, and a word for what has no text.
    fn notification_body(&self, message: &Message) -> String {
        let text = plain_text(&message.content, |kind, id| match kind {
            '@' => message
                .mentions
                .iter()
                .find(|u| u.id == id)
                .map(|u| format!("@{}", u.display_name())),
            '#' => self.spaces.channel(id).and_then(|c| c.name.clone()).map(|n| format!("#{n}")),
            '&' => message
                .guild_id
                .and_then(|g| self.spaces.guild(g))
                .and_then(|g| g.role(id))
                .map(|r| format!("@{}", r.name)),
            _ => None,
        });
        if !text.is_empty() {
            return text;
        }
        if !message.attachments.is_empty() {
            t!("notify.attachment").to_owned()
        } else if !message.sticker_items.is_empty() {
            t!("notify.sticker").to_owned()
        } else {
            t!("notify.message").to_owned()
        }
    }

    /// The count on the app icon: unread mentions everywhere plus incoming
    /// friend requests, as Discord's desktop app counts them. Sent only
    /// when it changes.
    #[hotpath::measure]
    pub(super) fn refresh_badge(&mut self) {
        let count = self.reads.all_mentions() + self.people.incoming_requests() as u32;
        if self.badge_shown != Some(count) {
            self.badge_shown = Some(count);
            self.alerts.badge(count);
        }
    }

    /// A notification was clicked: opens its channel.
    pub fn open_notified(&mut self, channel: Snowflake, window: &mut Window, cx: &mut Context<Self>) {
        self.close_overlay(window, cx);
        self.open_channel_link(None, channel, None, window, cx);
    }
}

/// `content` with Discord's markup tags made readable: `<@id>`, `<#id>`
/// and `<@&id>` through `name` (given the sigil `@`, `#` or `&`), custom
/// emoji as `:name:`, on one line and cut to [`MAX_BODY`] characters.
fn plain_text(content: &str, name: impl Fn(char, Snowflake) -> Option<String>) -> String {
    let mut out = String::new();
    let mut rest = content;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        let tail = &rest[start..];
        let Some(end) = tail.find('>') else {
            out.push_str(tail);
            rest = "";
            break;
        };
        let tag = &tail[1..end];
        out.push_str(&readable(tag, &name).unwrap_or_else(|| tail[..=end].to_owned()));
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    let line = out.split_whitespace().collect::<Vec<_>>().join(" ");
    match line.char_indices().nth(MAX_BODY) {
        Some((cut, _)) => format!("{}…", &line[..cut]),
        None => line,
    }
}

fn readable(tag: &str, name: &impl Fn(char, Snowflake) -> Option<String>) -> Option<String> {
    let id = |digits: &str| digits.parse().ok().map(Snowflake);
    if let Some(role) = tag.strip_prefix("@&") {
        return name('&', id(role)?);
    }
    if let Some(user) = tag.strip_prefix('@') {
        return name('@', id(user.trim_start_matches('!'))?);
    }
    if let Some(channel) = tag.strip_prefix('#') {
        return name('#', id(channel)?);
    }
    // `<:name:id>` and `<a:name:id>`.
    let emoji = tag.strip_prefix("a:").or_else(|| tag.strip_prefix(':'))?;
    let (emoji, _) = emoji.split_once(':')?;
    Some(format!(":{emoji}:"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markup_turns_into_names() {
        let name = |kind: char, id: Snowflake| match (kind, id.0) {
            ('@', 1) => Some("@Ana".to_owned()),
            ('#', 2) => Some("#geral".to_owned()),
            _ => None,
        };
        assert_eq!(
            plain_text("oi <@!1>, olha <#2>\n<:pog:3> <a:dance:4> <@9> <t:1:R>", name),
            "oi @Ana, olha #geral :pog: :dance: <@9> <t:1:R>"
        );
        assert_eq!(plain_text("a < b", name), "a < b");
        let long = "x".repeat(MAX_BODY + 5);
        assert_eq!(plain_text(&long, name).chars().count(), MAX_BODY + 1);
    }
}
