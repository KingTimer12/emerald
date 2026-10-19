//! The server list: the DM pseudo-space plus one space per guild, each with
//! its channels arranged into sidebar rows and, for guilds, what decides
//! permissions, nicknames and role colors.

use std::collections::{HashMap, HashSet};

use gpui::SharedString;

use super::super::super::icons;
use crate::discord::model::{Channel, ChannelKind, Guild, Member, Role, Snowflake};
use crate::discord::permissions::{self, GuildAccess};

/// Pseudo-space id holding direct messages.
pub const DMS: Snowflake = Snowflake(0);

pub enum Row {
    /// Entry point of the friends list, first row of the DM sidebar.
    Friends,
    /// Section title; guild categories carry their id so they can collapse.
    Category {
        id: Option<Snowflake>,
        title: SharedString,
    },
    Channel(ChannelRow),
}

pub struct ChannelRow {
    pub id: Snowflake,
    pub kind: ChannelKind,
    pub icon: &'static str,
    pub name: SharedString,
    pub parent: Option<Snowflake>,
    /// The other person in a 1:1 DM; drawn as their avatar.
    pub user: Option<Snowflake>,
}

impl ChannelRow {
    fn new(channel: &Channel) -> Self {
        Self {
            id: channel.id,
            kind: channel.kind,
            icon: channel_icon(channel.kind),
            name: channel.name.as_deref().unwrap_or("?").to_owned().into(),
            parent: channel.parent_id,
            user: channel.recipient(),
        }
    }
}

fn channel_icon(kind: ChannelKind) -> &'static str {
    match kind {
        ChannelKind::VOICE => icons::VOICE,
        ChannelKind::ANNOUNCEMENT => icons::ANNOUNCEMENT,
        ChannelKind::STAGE => icons::STAGE,
        ChannelKind::FORUM | ChannelKind::MEDIA => icons::FORUM,
        ChannelKind::GROUP_DM => icons::FRIENDS,
        _ => icons::TEXT,
    }
}

/// Guild data needed for permissions, names and role colors.
pub struct GuildInfo {
    /// The current user, whose permissions are computed.
    me: Snowflake,
    owner: Option<Snowflake>,
    roles: Vec<Role>,
    /// The current user's roles here.
    my_roles: Vec<Snowflake>,
    /// Nicknames and roles of message authors seen so far.
    members: HashMap<Snowflake, Member>,
    /// Authors already asked for with op 8, so each is requested once.
    requested: HashSet<Snowflake>,
    /// The current user's permissions per channel.
    permissions: HashMap<Snowflake, u64>,
}

impl GuildInfo {
    /// Display name and role color of a member, when known.
    pub fn member_style(&self, user: Snowflake) -> (Option<&str>, Option<u32>) {
        let Some(member) = self.members.get(&user) else { return (None, None) };
        let color = self
            .roles
            .iter()
            .filter(|role| role.color != 0 && member.roles.contains(&role.id))
            .max_by_key(|role| role.position)
            .map(|role| role.color);
        (member.nick.as_deref(), color)
    }

    pub fn role(&self, id: Snowflake) -> Option<&Role> {
        self.roles.iter().find(|r| r.id == id)
    }

    pub fn add_member(&mut self, user: Snowflake, member: Member) {
        self.members.insert(user, member);
    }

    /// The `authors` not known yet and not asked for before, now marked
    /// asked for.
    pub fn unrequested(&mut self, authors: impl Iterator<Item = Snowflake>) -> Vec<Snowflake> {
        let mut missing: Vec<Snowflake> = authors.filter(|id| !self.members.contains_key(id)).collect();
        missing.sort_unstable();
        missing.dedup();
        missing.retain(|id| self.requested.insert(*id));
        missing
    }
}

pub struct Space {
    pub id: Snowflake,
    pub name: SharedString,
    pub initials: SharedString,
    pub icon_url: Option<SharedString>,
    channels: Vec<Channel>,
    rows: Vec<Row>,
    /// `None` for the DM space.
    guild: Option<GuildInfo>,
}

impl Space {
    /// The DM space; `channels` must already be adopted by `People`.
    pub fn dms(mut channels: Vec<Channel>) -> Self {
        channels.retain(|c| c.kind.is_private());
        let mut space = Self {
            id: DMS,
            name: "Mensagens diretas".into(),
            initials: "DM".into(),
            icon_url: None,
            channels,
            rows: Vec::new(),
            guild: None,
        };
        space.rebuild();
        space
    }

    pub fn guild(guild: Guild, me: Snowflake, my_roles: Vec<Snowflake>) -> Self {
        let name = guild.display_name().to_owned();
        let mut space = Self {
            id: guild.id,
            initials: initials(&name).into(),
            icon_url: guild.icon_url().map(Into::into),
            name: name.into(),
            guild: Some(GuildInfo {
                me,
                owner: guild.owner(),
                roles: guild.roles,
                my_roles,
                members: HashMap::new(),
                requested: HashSet::new(),
                permissions: HashMap::new(),
            }),
            channels: guild.channels,
            rows: Vec::new(),
        };
        space.rebuild();
        space
    }

    pub fn is_dms(&self) -> bool {
        self.id == DMS
    }

    pub fn info(&self) -> Option<&GuildInfo> {
        self.guild.as_ref()
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn channels(&self) -> &[Channel] {
        &self.channels
    }

    /// The current user's permissions in `channel`; everything in DMs.
    pub fn permissions(&self, channel: Snowflake) -> u64 {
        match &self.guild {
            Some(guild) => guild.permissions.get(&channel).copied().unwrap_or(0),
            None => u64::MAX,
        }
    }

    /// Channels the current user can open and see.
    pub fn readable(&self) -> impl Iterator<Item = &Channel> {
        self.channels.iter().filter(|c| {
            c.kind.is_openable() && self.permissions(c.id) & permissions::VIEW_CHANNEL != 0
        })
    }

    pub fn row(&self, id: Snowflake) -> Option<&ChannelRow> {
        self.rows.iter().find_map(|row| match row {
            Row::Channel(channel) if channel.id == id => Some(channel),
            _ => None,
        })
    }

    pub fn first_openable(&self) -> Option<Snowflake> {
        self.rows.iter().find_map(|row| match row {
            Row::Channel(channel) if channel.kind.is_openable() => Some(channel.id),
            _ => None,
        })
    }

    pub fn topic(&self, id: Snowflake) -> Option<&str> {
        self.channels
            .iter()
            .find(|c| c.id == id)
            .and_then(|c| c.topic.as_deref())
            .filter(|t| !t.is_empty())
    }

    /// Indices of the rows whose channel name contains `query`; all rows
    /// for an empty query.
    pub fn search(&self, query: &str) -> Vec<usize> {
        let query = query.trim().to_lowercase();
        self.matching(|row| match row {
            _ if query.is_empty() => true,
            Row::Channel(channel) => channel.name.to_lowercase().contains(&query),
            _ => false,
        })
    }

    /// Indices of the rows outside collapsed categories, plus those `keep`
    /// pins in place (the selected and unread channels, as on Discord).
    pub fn expanded(&self, collapsed: &HashSet<Snowflake>, keep: impl Fn(Snowflake) -> bool) -> Vec<usize> {
        self.matching(|row| match row {
            Row::Channel(ChannelRow {
                id,
                parent: Some(parent),
                ..
            }) => !collapsed.contains(parent) || keep(*id),
            _ => true,
        })
    }

    fn matching(&self, keep: impl Fn(&Row) -> bool) -> Vec<usize> {
        (0..self.rows.len()).filter(|ix| keep(&self.rows[*ix])).collect()
    }

    pub fn upsert_channel(&mut self, channel: Channel) {
        self.channels.retain(|c| c.id != channel.id);
        self.channels.push(channel);
        self.rebuild();
    }

    pub fn remove_channel(&mut self, id: Snowflake) {
        self.channels.retain(|c| c.id != id);
        self.rebuild();
    }

    /// Records `message` as the newest in `channel`; false when the channel
    /// is not here.
    pub fn touch(&mut self, channel: Snowflake, message: Snowflake) -> bool {
        let Some(found) = self.channels.iter_mut().find(|c| c.id == channel) else {
            return false;
        };
        found.last_message_id = Some(message);
        // Only the DM list is ordered by activity.
        if self.guild.is_none() {
            self.rebuild();
        }
        true
    }

    fn rebuild(&mut self) {
        self.rows.clear();
        let Some(guild) = &mut self.guild else {
            self.channels
                .sort_unstable_by_key(|c| std::cmp::Reverse(c.activity()));
            self.rows.push(Row::Friends);
            self.rows.push(Row::Category {
                id: None,
                title: "MENSAGENS DIRETAS".into(),
            });
            self.rows
                .extend(self.channels.iter().map(|c| Row::Channel(ChannelRow::new(c))));
            return;
        };

        let access = GuildAccess {
            guild_id: self.id,
            owner: guild.owner,
            roles: &guild.roles,
            me: guild.me,
            my_roles: &guild.my_roles,
        };
        guild.permissions = self
            .channels
            .iter()
            .map(|c| (c.id, access.channel(&c.permission_overwrites)))
            .collect();
        let permissions = &guild.permissions;
        let visible = |c: &&Channel| {
            c.kind.is_listed() && permissions.get(&c.id).is_some_and(|p| p & permissions::VIEW_CHANNEL != 0)
        };
        // Discord lists text-like channels before voice-like ones, then by
        // position.
        let order = |c: &&Channel| (c.kind.is_voice(), c.position, c.id);
        let children = |parent: Option<Snowflake>| {
            let mut children: Vec<_> = self
                .channels
                .iter()
                .filter(|c| visible(c) && c.parent_id == parent)
                .collect();
            children.sort_unstable_by_key(order);
            children
        };

        let loose = children(None);
        self.rows
            .extend(loose.into_iter().map(|c| Row::Channel(ChannelRow::new(c))));
        let mut categories: Vec<_> = self
            .channels
            .iter()
            .filter(|c| c.kind == ChannelKind::CATEGORY)
            .collect();
        categories.sort_unstable_by_key(|c| (c.position, c.id));
        for category in categories {
            let children = children(Some(category.id));
            // Categories with nothing the user can see are hidden, as on Discord.
            if children.is_empty() {
                continue;
            }
            let title = category.name.as_deref().unwrap_or_default().to_uppercase();
            self.rows.push(Row::Category {
                id: Some(category.id),
                title: title.into(),
            });
            self.rows
                .extend(children.into_iter().map(|c| Row::Channel(ChannelRow::new(c))));
        }
    }
}

/// All spaces in sidebar order, the selected one, and per-space choices the
/// user made that outlive a reconnect.
pub struct Spaces {
    list: Vec<Space>,
    selected: usize,
    /// Collapsed guild categories.
    collapsed: HashSet<Snowflake>,
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
            collapsed: HashSet::new(),
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

    pub fn is_collapsed(&self, category: Snowflake) -> bool {
        self.collapsed.contains(&category)
    }

    pub fn collapsed(&self) -> &HashSet<Snowflake> {
        &self.collapsed
    }

    pub fn toggle_collapsed(&mut self, category: Snowflake) {
        if !self.collapsed.remove(&category) {
            self.collapsed.insert(category);
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
    use crate::discord::model::{Overwrite, overwrite_kind};

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
    fn collapsed_categories_keep_pinned_channels() {
        let space = guild(vec![
            channel(1, ChannelKind::CATEGORY, None, 0),
            channel(2, ChannelKind::GUILD_TEXT, Some(1), 0),
            channel(3, ChannelKind::GUILD_TEXT, Some(1), 1),
        ]);
        let collapsed = HashSet::from([Snowflake(1)]);
        let rows = space.expanded(&collapsed, |id| id == Snowflake(3));
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
        assert_eq!(names(&space, &all), ["friends", "MENSAGENS DIRETAS", "Bruno", "Ana"]);
        assert!(space.touch(Snowflake(1), Snowflake(30)));
        let found = space.search("an");
        assert_eq!(names(&space, &found), ["Ana"]);
        assert_eq!(space.search("").get(2).map(|ix| names(&space, &[*ix])), Some(vec!["Ana".into()]));
    }
}
