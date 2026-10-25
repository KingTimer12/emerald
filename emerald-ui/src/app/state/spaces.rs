//! The server list: the DM pseudo-space plus one space per guild, each with
//! its channels arranged into sidebar rows and, for guilds, what decides
//! permissions, nicknames and role colors.

mod guild_info;
mod rows;
mod space;

pub use guild_info::*;
pub use rows::*;
pub use space::*;

use std::collections::{HashMap, HashSet};

use gpui::SharedString;

use super::super::super::icons;
use emerald_discord::model::{
    Channel, ChannelKind, Guild, GuildEmoji, Member, Role, Snowflake, StickerItem, ThreadListSync, ThreadMember,
};
use emerald_discord::permissions::{self, GuildAccess};

/// Pseudo-space id holding direct messages.
pub const DMS: Snowflake = Snowflake(0);

/// All spaces in sidebar order, the selected one, and per-space choices the
/// user made that outlive a reconnect.
pub struct Spaces {
    list: Vec<Space>,
    selected: usize,
    /// Last channel opened per guild.
    last_channel: HashMap<Snowflake, Snowflake>,
    /// Guilds already subscribed to with op 37 this session.
    subscribed: HashSet<Snowflake>,
}

impl Default for Spaces {
    fn default() -> Self {
        Self {
            list: vec![Space::dms(Vec::new())],
            selected: 0,
            last_channel: HashMap::new(),
            subscribed: HashSet::new(),
        }
    }
}

impl Spaces {
    /// Replaces the list for a new session, keeping the selected space when
    /// it still exists. Subscriptions belong to the old session.
    pub fn reset(&mut self, list: Vec<Space>) {
        let previous = self.current().id;
        self.list = list;
        self.selected = self.index(previous).unwrap_or(0);
        self.subscribed.clear();
    }

    /// Puts the guilds in `order` (the server list's, folders flattened),
    /// keeping the DMs first and the selected space selected. Guilds
    /// missing from `order` keep their place before the rest.
    pub fn reorder(&mut self, order: &[Snowflake]) {
        let selected = self.current().id;
        let rank = |id: Snowflake| order.iter().position(|o| *o == id).map_or(0, |ix| ix + 1);
        self.list[1..].sort_by_key(|space| rank(space.id));
        self.selected = self.index(selected).unwrap_or(0);
    }

    /// The guilds in list order.
    pub fn guild_ids(&self) -> Vec<Snowflake> {
        self.list[1..].iter().map(|s| s.id).collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Space> {
        self.list.iter()
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn current(&self) -> &Space {
        &self.list[self.selected]
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn select(&mut self, ix: usize) {
        self.selected = ix.min(self.list.len() - 1);
    }

    pub fn select_dms(&mut self) {
        self.selected = 0;
    }

    pub fn index(&self, id: Snowflake) -> Option<usize> {
        self.list.iter().position(|s| s.id == id)
    }

    pub fn get_mut(&mut self, id: Snowflake) -> Option<&mut Space> {
        self.list.iter_mut().find(|s| s.id == id)
    }

    pub fn dms(&self) -> &Space {
        &self.list[0]
    }

    pub fn dms_mut(&mut self) -> &mut Space {
        &mut self.list[0]
    }

    pub fn guild(&self, id: Snowflake) -> Option<&GuildInfo> {
        self.list.iter().find(|s| s.id == id)?.guild.as_ref()
    }

    /// Words the DM space's own labels again, in a new language.
    pub fn relabel(&mut self) {
        let dms = self.dms_mut();
        dms.name = t!("dm.title").into();
        dms.rebuild();
    }

    pub fn guild_mut(&mut self, id: Snowflake) -> Option<&mut GuildInfo> {
        self.get_mut(id)?.guild.as_mut()
    }

    /// Any channel the client knows, in any space.
    pub fn channel(&self, id: Snowflake) -> Option<&Channel> {
        self.list.iter().find_map(|s| s.channels.iter().find(|c| c.id == id))
    }

    /// The guild holding `channel`.
    pub fn guild_of(&mut self, channel: Snowflake) -> Option<(Snowflake, &mut GuildInfo)> {
        let space = self
            .list
            .iter_mut()
            .find(|s| s.guild.is_some() && s.channels.iter().any(|c| c.id == channel))?;
        Some((space.id, space.guild.as_mut()?))
    }

    /// The id of the guild listing `channel`, if a guild does.
    pub fn guild_id_of(&self, channel: Snowflake) -> Option<Snowflake> {
        self.list
            .iter()
            .find(|s| s.guild.is_some() && s.channels.iter().any(|c| c.id == channel))
            .map(|s| s.id)
    }

    /// The open 1:1 DM with `user`, if any.
    pub fn dm_with(&self, user: Snowflake) -> Option<Snowflake> {
        self.dms().channels.iter().find(|c| c.recipient() == Some(user)).map(|c| c.id)
    }

    /// GUILD_CREATE: replaces a known guild, keeping the authors already
    /// resolved, or appends a new one. Returns the new guild's index.
    pub fn upsert_guild(&mut self, guild: Guild, me: Snowflake) -> Option<usize> {
        let existing = self.index(guild.id);
        // The current member comes with the guild, or from before.
        let my_roles = guild
            .members
            .iter()
            .find(|m| m.id() == Some(me))
            .map(|m| m.roles.clone())
            .or_else(|| existing.and_then(|ix| self.list[ix].guild.as_ref().map(|g| g.my_roles.clone())))
            .unwrap_or_default();
        let mut space = Space::guild(guild, me, my_roles);
        match existing {
            Some(ix) => {
                if let (Some(old), Some(new)) = (self.list[ix].guild.take(), space.guild.as_mut()) {
                    new.members = old.members;
                    new.requested = old.requested;
                }
                self.list[ix] = space;
                None
            }
            None => {
                self.list.push(space);
                Some(self.list.len() - 1)
            }
        }
    }

    pub fn remove(&mut self, id: Snowflake) {
        if let Some(ix) = self.index(id).filter(|ix| *ix > 0) {
            self.list.remove(ix);
            if self.selected >= ix {
                self.selected = self.selected.saturating_sub(1);
            }
        }
    }

    /// Marks `guild` subscribed; true the first time this session.
    pub fn subscribe(&mut self, guild: Snowflake) -> bool {
        self.subscribed.insert(guild)
    }

    pub fn remember_channel(&mut self, channel: Snowflake) {
        let space = self.current().id;
        self.last_channel.insert(space, channel);
    }

    /// The channel to open with the current space: the last one opened
    /// there if still listed, else the first the user can read.
    pub fn landing_channel(&self) -> Option<Snowflake> {
        let space = self.current();
        self.last_channel
            .get(&space.id)
            .copied()
            .filter(|id| space.row(*id).is_some())
            .or_else(|| space.first_openable())
    }
}

fn initials(name: &str) -> String {
    let initials: String = name
        .split_whitespace()
        .filter_map(|word| word.chars().next())
        .take(3)
        .collect();
    if initials.is_empty() { "?".into() } else { initials }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emerald_discord::model::{Overwrite, overwrite_kind};

    const GUILD: Snowflake = Snowflake(100);

    fn channel(id: u64, kind: ChannelKind, parent: Option<u64>, position: i32) -> Channel {
        Channel {
            id: Snowflake(id),
            kind,
            name: Some(format!("c{id}")),
            position,
            parent_id: parent.map(Snowflake),
            ..Channel::default()
        }
    }

    fn guild(channels: Vec<Channel>) -> Space {
        let everyone = Role {
            id: GUILD,
            name: "@everyone".into(),
            color: 0,
            position: 0,
            permissions: permissions::VIEW_CHANNEL,
            mentionable: false,
        };
        let guild = serde_json::from_value::<Guild>(serde_json::json!({ "id": "100", "name": "Rust Brasil" }))
            .map(|mut g| {
                g.channels = channels;
                g.roles = vec![everyone];
                g
            })
            .unwrap();
        Space::guild(guild, Snowflake(1), Vec::new())
    }

    fn names(space: &Space, rows: &[usize]) -> Vec<String> {
        rows.iter()
            .map(|ix| match &space.rows()[*ix] {
                Row::Channel(c) => c.name.to_string(),
                Row::Category { title, .. } => title.to_string(),
                Row::Friends => "friends".into(),
                Row::Requests => "requests".into(),
            })
            .collect()
    }

    #[test]
    fn guild_rows_put_text_first_and_hide_what_cannot_be_seen() {
        let mut hidden = channel(5, ChannelKind::GUILD_TEXT, Some(2), 0);
        hidden.permission_overwrites = vec![Overwrite {
            id: GUILD,
            kind: overwrite_kind::ROLE,
            allow: 0,
            deny: permissions::VIEW_CHANNEL,
        }];
        let space = guild(vec![
            channel(1, ChannelKind::CATEGORY, None, 0),
            channel(2, ChannelKind::CATEGORY, None, 1),
            channel(3, ChannelKind::VOICE, Some(1), 0),
            channel(4, ChannelKind::GUILD_TEXT, Some(1), 5),
            hidden,
            channel(6, ChannelKind::GUILD_TEXT, None, 9),
        ]);
        let all: Vec<usize> = (0..space.rows().len()).collect();
        assert_eq!(names(&space, &all), ["c6", "C1", "c4", "c3"]);
        assert_eq!(space.initials.as_ref(), "RB");
        assert_eq!(space.first_openable(), Some(Snowflake(6)));
    }

    #[test]
    fn role_and_member_updates_change_access() {
        let mut space = guild(vec![channel(1, ChannelKind::GUILD_TEXT, None, 0)]);
        assert_ne!(space.permissions(Snowflake(1)) & permissions::VIEW_CHANNEL, 0);
        // @everyone loses the channel; a new role given to the user brings it back.
        let role = |id: u64, permissions: u64| Role {
            id: Snowflake(id),
            name: format!("r{id}"),
            color: 0xff0000,
            position: 1,
            permissions,
            mentionable: false,
        };
        space.upsert_role(role(100, 0));
        assert_eq!(space.permissions(Snowflake(1)), 0);
        assert!(space.rows().is_empty());
        space.upsert_role(role(7, permissions::VIEW_CHANNEL));
        let member = |roles: Vec<Snowflake>| Member {
            user_id: Some(Snowflake(1)),
            user: None,
            nick: Some("eu".into()),
            roles,
        };
        space.update_member(Snowflake(1), member(vec![Snowflake(7)]));
        assert_ne!(space.permissions(Snowflake(1)) & permissions::VIEW_CHANNEL, 0);
        let info = space.info().unwrap();
        assert_eq!(info.member_style(Snowflake(1)), (Some("eu"), Some(0xff0000)));
        space.remove_role(Snowflake(7));
        assert_eq!(space.permissions(Snowflake(1)), 0);
        assert!(space.info().unwrap().my_roles().is_empty());
    }

    #[test]
    fn collapsed_categories_keep_pinned_channels() {
        let space = guild(vec![
            channel(1, ChannelKind::CATEGORY, None, 0),
            channel(2, ChannelKind::GUILD_TEXT, Some(1), 0),
            channel(3, ChannelKind::GUILD_TEXT, Some(1), 1),
        ]);
        let rows = space.expanded(|category| category == Snowflake(1), |id| id == Snowflake(3));
        assert_eq!(names(&space, &rows), ["C1", "c3"]);
    }

    #[test]
    fn dm_search_and_activity_order() {
        let dm = |id: u64, last: u64, name: &str| Channel {
            id: Snowflake(id),
            kind: ChannelKind::DM,
            name: Some(name.into()),
            last_message_id: Some(Snowflake(last)),
            ..Channel::default()
        };
        let mut space = Space::dms(vec![dm(1, 10, "Ana"), dm(2, 20, "Bruno")]);
        let all = space.search("");
        assert_eq!(names(&space, &all), ["friends", t!("dm.header"), "Bruno", "Ana"]);
        assert!(space.touch(Snowflake(1), Snowflake(30)));
        let found = space.search("an");
        assert_eq!(names(&space, &found), ["Ana"]);
        assert_eq!(space.search("").get(2).map(|ix| names(&space, &[*ix])), Some(vec!["Ana".into()]));
    }

    #[test]
    fn message_requests_stay_out_of_the_dm_list() {
        let dm = |id: u64, request: bool, spam: bool| Channel {
            id: Snowflake(id),
            kind: ChannelKind::DM,
            name: Some(format!("d{id}")),
            is_message_request: request,
            is_spam: spam,
            ..Channel::default()
        };
        let mut space = Space::dms(vec![dm(1, false, false), dm(2, true, false), dm(3, true, true)]);
        let all = space.search("");
        assert_eq!(names(&space, &all), ["friends", "requests", t!("dm.header"), "d1"]);
        assert_eq!(space.requests(false).map(|c| c.id.0).collect::<Vec<_>>(), [2]);
        assert_eq!(space.requests(true).map(|c| c.id.0).collect::<Vec<_>>(), [3]);
        space.label_dms(|c| format!("label {}", c.id));
        assert_eq!(space.label(Snowflake(1)).map(|l| l.as_ref()), Some("label 1"));
        assert_eq!(names(&space, &space.search("")).last().map(String::as_str), Some("label 1"));
    }

    #[test]
    fn guild_channels_are_found_without_their_guild_id() {
        let mut spaces = Spaces::default();
        let text = channel(1, ChannelKind::GUILD_TEXT, None, 0);
        spaces.reset(vec![Space::dms(Vec::new()), guild(vec![text])]);
        assert_eq!(spaces.guild_id_of(Snowflake(1)), Some(GUILD));
        assert_eq!(spaces.guild_id_of(Snowflake(2)), None);
    }
}
