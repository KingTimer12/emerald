//! Everyone the client knows about: the signed-in user, the users seen in
//! payloads, relationships (friends, requests, blocks) with their
//! nicknames, who is ignored, notes and presences.

use rustc_hash::{FxHashMap, FxHashSet};

use gpui::SharedString;

use emerald_discord::model::{
    Activity, Channel, FriendPresence, Relationship, Snowflake, User, avatar_url, relationship_kind,
};

#[derive(Default)]
pub struct People {
    me: Option<User>,
    users: FxHashMap<Snowflake, User>,
    /// Relationship kind per user (`relationship_kind`).
    relationships: FxHashMap<Snowflake, u8>,
    /// Friend presences; missing means offline.
    presences: FxHashMap<Snowflake, SharedString>,
    /// What friends are doing; missing means nothing.
    activities: FxHashMap<Snowflake, Vec<Activity>>,
    /// Names the user gave their friends.
    nicknames: FxHashMap<Snowflake, String>,
    /// Users whose messages fold away, short of blocking them.
    ignored: FxHashSet<Snowflake>,
    /// The user's notes on profiles, as read so far; empty for none.
    notes: FxHashMap<Snowflake, String>,
}

impl People {
    /// Replaces everything with what a new session reports.
    pub fn load(&mut self, me: User, users: Vec<User>, relationships: Vec<Relationship>) {
        self.users = users.into_iter().map(|u| (u.id, u)).collect();
        self.relationships.clear();
        self.nicknames.clear();
        self.ignored.clear();
        // Notes come back with each profile; another device may have
        // changed them meanwhile.
        self.notes.clear();
        for relationship in relationships {
            self.add_relationship(relationship);
        }
        self.me = Some(me);
    }

    pub fn me(&self) -> Option<&User> {
        self.me.as_ref()
    }

    pub fn my_id(&self) -> Snowflake {
        self.me.as_ref().map_or(Snowflake(0), |me| me.id)
    }

    pub fn is_me(&self, id: Snowflake) -> bool {
        self.me.as_ref().is_some_and(|me| me.id == id)
    }

    pub fn user(&self, id: Snowflake) -> Option<&User> {
        self.users.get(&id)
    }

    /// A user whose username or display name is `name`, ignoring case;
    /// the signed-in user included.
    pub fn find(&self, name: &str) -> Option<Snowflake> {
        let name = name.to_lowercase();
        self.me
            .iter()
            .chain(self.users.values())
            .find(|u| u.username.to_lowercase() == name || u.global_name.as_ref().is_some_and(|g| g.to_lowercase() == name))
            .map(|u| u.id)
    }

    pub fn display_name(&self, id: Snowflake) -> Option<&str> {
        self.user(id).map(User::display_name)
    }

    /// Avatar URL of a known user, or the default one their id maps to.
    pub fn avatar_url(&self, id: Snowflake) -> String {
        match self.users.get(&id) {
            Some(user) => user.avatar_url(),
            None => avatar_url(id, None),
        }
    }

    pub fn remember(&mut self, user: User) {
        self.users.insert(user.id, user);
    }

    /// Keeps the first copy of `user`; payload copies are often partial.
    pub fn remember_new(&mut self, user: &User) {
        self.users.entry(user.id).or_insert_with(|| user.clone());
    }

    /// USER_UPDATE. Updates of the current user never carry the email again,
    /// so the known one is kept.
    pub fn update(&mut self, user: User) {
        if let Some(me) = self.me.as_mut().filter(|me| me.id == user.id) {
            let email = me.email.take();
            *me = user.clone();
            me.email = me.email.take().or(email);
        }
        self.remember(user);
    }

    pub fn presence(&self, id: Snowflake) -> &str {
        self.presences.get(&id).map_or("offline", |s| s.as_ref())
    }

    pub fn status(&self, id: Snowflake) -> Option<&SharedString> {
        self.presences.get(&id)
    }

    /// A friend's activities, custom status included.
    pub fn activities(&self, id: Snowflake) -> &[Activity] {
        self.activities.get(&id).map_or(&[], Vec::as_slice)
    }

    #[hotpath::measure]
    pub fn set_presences(&mut self, presences: Vec<FriendPresence>) {
        for FriendPresence { user, status, activities } in presences {
            let offline = status == "offline";
            if offline {
                self.presences.remove(&user);
            } else {
                self.presences.insert(user, status.into());
            }
            if activities.is_empty() || offline {
                self.activities.remove(&user);
            } else {
                self.activities.insert(user, activities);
            }
        }
    }

    /// RELATIONSHIP_ADD and _UPDATE; true when it made the user a friend
    /// just now.
    pub fn add_relationship(&mut self, relationship: Relationship) -> bool {
        let id = relationship.id;
        let previous = self.relationships.insert(id, relationship.kind);
        match relationship.nickname.filter(|n| !n.is_empty()) {
            Some(nickname) => self.nicknames.insert(id, nickname),
            None => self.nicknames.remove(&id),
        };
        if relationship.user_ignored {
            self.ignored.insert(id);
        } else {
            self.ignored.remove(&id);
        }
        if let Some(user) = relationship.user {
            self.remember(user);
        }
        relationship.kind == relationship_kind::FRIEND && previous != Some(relationship_kind::FRIEND)
    }

    pub fn remove_relationship(&mut self, id: Snowflake) {
        self.relationships.remove(&id);
        self.nicknames.remove(&id);
        self.ignored.remove(&id);
    }

    /// The relationship kind with `id`, if any (`relationship_kind`).
    pub fn relationship(&self, id: Snowflake) -> Option<u8> {
        self.relationships.get(&id).copied()
    }

    pub fn is_friend(&self, id: Snowflake) -> bool {
        self.relationship(id) == Some(relationship_kind::FRIEND)
    }

    pub fn is_blocked(&self, id: Snowflake) -> bool {
        self.relationship(id) == Some(relationship_kind::BLOCKED)
    }

    /// Ignored and not blocked, as the web client counts it.
    pub fn is_ignored(&self, id: Snowflake) -> bool {
        self.ignored.contains(&id) && !self.is_blocked(id)
    }

    /// Whether `id`'s messages fold away in the chat.
    pub fn is_hidden(&self, id: Snowflake) -> bool {
        self.is_blocked(id) || self.ignored.contains(&id)
    }

    pub fn nickname(&self, id: Snowflake) -> Option<&str> {
        self.nicknames.get(&id).map(String::as_str)
    }

    /// The name a friend goes by here: the nickname the user gave them,
    /// else their display name.
    pub fn friend_name(&self, id: Snowflake) -> Option<&str> {
        self.nickname(id).or_else(|| self.display_name(id))
    }

    /// The note on `id`'s profile; `None` until read.
    pub fn note(&self, id: Snowflake) -> Option<&str> {
        self.notes.get(&id).map(String::as_str)
    }

    pub fn set_note(&mut self, id: Snowflake, note: String) {
        self.notes.insert(id, note);
    }

    pub fn relationships(&self) -> impl Iterator<Item = (Snowflake, u8)> + '_ {
        self.relationships.iter().map(|(id, kind)| (*id, *kind))
    }

    pub fn incoming_requests(&self) -> usize {
        self.relationships
            .values()
            .filter(|kind| **kind == relationship_kind::INCOMING)
            .count()
    }

    /// Moves a DM's embedded recipients into the user table, keeping only
    /// their ids.
    pub fn adopt_dm(&mut self, channel: &mut Channel) {
        if !channel.recipients.is_empty() {
            channel.recipient_ids = channel.recipients.iter().map(|u| u.id).collect();
            for user in channel.recipients.drain(..) {
                self.users.insert(user.id, user);
            }
        }
    }

    /// What a DM is called: its name when a group has one, else its
    /// people, by the nicknames the user gave them.
    pub fn dm_name(&self, channel: &Channel) -> String {
        if let Some(name) = channel.name.as_deref().filter(|n| !n.is_empty()) {
            return name.to_owned();
        }
        let names: Vec<&str> = channel.recipient_ids.iter().filter_map(|id| self.friend_name(*id)).collect();
        if names.is_empty() { "?".into() } else { names.join(", ") }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use emerald_discord::model::ChannelKind;

    fn user(id: u64, name: &str) -> User {
        User {
            id: Snowflake(id),
            username: name.into(),
            global_name: None,
            avatar: None,
            email: None,
            bot: false,
            premium_type: None,
        }
    }

    #[test]
    fn befriending_is_reported_once() {
        let mut people = People::default();
        let friend = |kind| Relationship {
            id: Snowflake(2),
            kind,
            user: None,
            nickname: None,
            user_ignored: false,
        };
        assert!(!people.add_relationship(friend(relationship_kind::OUTGOING)));
        assert!(people.add_relationship(friend(relationship_kind::FRIEND)));
        assert!(!people.add_relationship(friend(relationship_kind::FRIEND)));
    }

    #[test]
    fn own_updates_keep_the_email() {
        let mut people = People::default();
        let mut me = user(1, "eu");
        me.email = Some("a@b.c".into());
        people.load(me, Vec::new(), Vec::new());
        people.update(user(1, "novo"));
        let me = people.me().unwrap();
        assert_eq!((me.username.as_str(), me.email.as_deref()), ("novo", Some("a@b.c")));
    }

    #[test]
    fn unnamed_dms_are_named_after_recipients() {
        let mut people = People::default();
        let mut dm = Channel {
            kind: ChannelKind::GROUP_DM,
            recipients: vec![user(2, "ana"), user(3, "bia")],
            ..Channel::default()
        };
        people.adopt_dm(&mut dm);
        assert!(dm.recipients.is_empty());
        assert_eq!(people.display_name(Snowflake(3)), Some("bia"));
        people.add_relationship(Relationship {
            id: Snowflake(3),
            kind: relationship_kind::FRIEND,
            user: None,
            nickname: Some("Bibi".into()),
            user_ignored: false,
        });
        assert_eq!(people.dm_name(&dm), "ana, Bibi");
        dm.name = Some("Grupo".into());
        assert_eq!(people.dm_name(&dm), "Grupo");
    }

    #[test]
    fn blocked_and_ignored_users_are_hidden() {
        let mut people = People::default();
        let relationship = |id, kind, ignored| Relationship {
            id: Snowflake(id),
            kind,
            user: None,
            nickname: None,
            user_ignored: ignored,
        };
        people.add_relationship(relationship(2, relationship_kind::BLOCKED, false));
        people.add_relationship(relationship(3, 0, true));
        assert!(people.is_blocked(Snowflake(2)) && people.is_hidden(Snowflake(2)));
        assert!(people.is_ignored(Snowflake(3)) && people.is_hidden(Snowflake(3)));
        people.remove_relationship(Snowflake(3));
        assert!(!people.is_hidden(Snowflake(3)));
    }
}
