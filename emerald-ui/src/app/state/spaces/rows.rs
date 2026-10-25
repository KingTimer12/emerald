//! Sidebar rows: how a space's channels, categories and threads are laid out.

use super::*;

pub enum Row {
    /// Entry point of the friends list, first row of the DM sidebar.
    Friends,
    /// "Message Requests", under it while any wait.
    Requests,
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
    /// A group DM's picture, and how many are in it.
    pub picture: Option<SharedString>,
    pub members: Option<usize>,
}

impl ChannelRow {
    pub(super) fn new(channel: &Channel) -> Self {
        let group = channel.kind == ChannelKind::GROUP_DM;
        Self {
            id: channel.id,
            kind: channel.kind,
            icon: channel_icon(channel.kind),
            name: channel.name.as_deref().unwrap_or("?").to_owned().into(),
            parent: channel.parent_id,
            user: channel.recipient(),
            picture: group.then(|| channel.group_icon_url().into()),
            // The recipients leave the user out.
            members: group.then_some(channel.recipient_ids.len() + 1),
        }
    }
}

pub(super) fn channel_icon(kind: ChannelKind) -> &'static str {
    match kind {
        ChannelKind::VOICE => icons::VOICE,
        ChannelKind::ANNOUNCEMENT => icons::ANNOUNCEMENT,
        ChannelKind::STAGE => icons::STAGE,
        ChannelKind::FORUM | ChannelKind::MEDIA => icons::FORUM,
        ChannelKind::GROUP_DM => icons::FRIENDS,
        kind if kind.is_thread() => icons::THREAD,
        _ => icons::TEXT,
    }
}

impl Space {
    pub(super) fn rebuild(&mut self) {
        self.rows.clear();
        let Some(guild) = &mut self.guild else {
            self.channels
                .sort_unstable_by_key(|c| std::cmp::Reverse(c.activity()));
            self.rows.push(Row::Friends);
            if self.channels.iter().any(|c| c.is_message_request) {
                self.rows.push(Row::Requests);
            }
            self.rows.push(Row::Category {
                id: None,
                title: t!("dm.header").into(),
            });
            let labels = &self.labels;
            self.rows.extend(self.channels.iter().filter(|c| !c.is_message_request).map(|c| {
                let mut row = ChannelRow::new(c);
                if let Some(label) = labels.get(&c.id) {
                    row.name = label.clone();
                }
                Row::Channel(row)
            }));
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
            .filter(|c| !c.kind.is_thread())
            .map(|c| (c.id, access.channel(&c.permission_overwrites)))
            .collect();
        for thread in self.channels.iter().filter(|c| c.kind.is_thread()) {
            let parent = thread.parent_id.and_then(|p| guild.permissions.get(&p)).copied().unwrap_or(0);
            let private = thread.kind == ChannelKind::PRIVATE_THREAD;
            let allowed = permissions::thread(parent, private, thread.member.is_some(), thread.is_locked());
            guild.permissions.insert(thread.id, allowed);
        }
        let permissions = &guild.permissions;
        // Under a text channel: the active threads we joined, newest join
        // first, and the open one first of all, as the web client lists them.
        let shown = self.shown_thread;
        let threads = |parent: Snowflake| {
            let mut threads: Vec<&Channel> = self
                .channels
                .iter()
                .filter(|c| c.parent_id == Some(parent) && c.is_active_thread())
                .filter(|c| c.member.is_some() || Some(c.id) == shown)
                .filter(|c| permissions.get(&c.id).is_some_and(|p| p & permissions::VIEW_CHANNEL != 0))
                .collect();
            threads.sort_by(|a, b| {
                let joined = |c: &Channel| c.member.as_ref().and_then(|m| m.join_timestamp.clone());
                (Some(b.id) == shown).cmp(&(Some(a.id) == shown)).then_with(|| joined(b).cmp(&joined(a)))
            });
            threads
        };
        let with_threads = |channels: Vec<&Channel>| {
            channels
                .into_iter()
                .flat_map(|c| {
                    let nested = if c.kind.is_forum() { Vec::new() } else { threads(c.id) };
                    // Threads fold away with their parent's category.
                    let nested = nested.into_iter().map(|t| ChannelRow { parent: c.parent_id, ..ChannelRow::new(t) });
                    std::iter::once(ChannelRow::new(c)).chain(nested).collect::<Vec<_>>()
                })
                .map(Row::Channel)
                .collect::<Vec<_>>()
        };
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

        let loose = with_threads(children(None));
        self.rows.extend(loose);
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
            self.rows.extend(with_threads(children));
        }
    }
}
