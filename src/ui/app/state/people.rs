//! Everyone the client knows about: the signed-in user, the users seen in
//! payloads, friend relationships and presences.

use std::collections::HashMap;

use gpui::SharedString;

use crate::discord::model::{Channel, Relationship, Snowflake, User, avatar_url, relationship_kind};

#[derive(Default)]
pub struct People {
    me: Option<User>,
    users: HashMap<Snowflake, User>,
    /// Relationship kind per user (`relationship_kind`).
    relationships: HashMap<Snowflake, u8>,
    /// Friend presences; missing means offline.
    presences: HashMap<Snowflake, SharedString>,
}

impl People {
    /// Replaces everything with what a new session reports.
    pub fn load(&mut self, me: User, users: Vec<User>, relationships: Vec<Relationship>) {
        self.users = users.into_iter().map(|u| (u.id, u)).collect();
        self.relationships.clear();
        for relationship in relationships {
            self.relationships.insert(relationship.id, relationship.kind);
            if let Some(user) = relationship.user {
                self.users.insert(user.id, user);
            }
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

    pub fn set_presences(&mut self, presences: Vec<(Snowflake, String)>) {
        for (user, status) in presences {
            if status == "offline" {
                self.presences.remove(&user);
            } else {
                self.presences.insert(user, status.into());
            }
        }
    }

    /// RELATIONSHIP_ADD; true when it made the user a friend just now.
    pub fn add_relationship(&mut self, relationship: Relationship) -> bool {
        let previous = self.relationships.insert(relationship.id, relationship.kind);
        if let Some(user) = relationship.user {
            self.remember(user);
        }
        relationship.kind == relationship_kind::FRIEND && previous != Some(relationship_kind::FRIEND)
    }

    pub fn remove_relationship(&mut self, id: Snowflake) {
        self.relationships.remove(&id);
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
    /// their ids, and names the channel after them.
    pub fn adopt_dm(&mut self, channel: &mut Channel) {
        if !channel.recipients.is_empty() {
            channel.recipient_ids = channel.recipients.iter().map(|u| u.id).collect();
            for user in channel.recipients.drain(..) {
                self.users.insert(user.id, user);
            }
        }
        if channel.name.as_deref().is_none_or(str::is_empty) {
            let names: Vec<&str> = channel
                .recipient_ids
                .iter()
                .filter_map(|id| self.display_name(*id))
                .collect();
            channel.name = Some(if names.is_empty() { "?".into() } else { names.join(", ") });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discord::model::ChannelKind;

    fn user(id: u64, name: &str) -> User {
        User {
            id: Snowflake(id),
            username: name.into(),
            global_name: None,
            avatar: None,
            email: None,
            bot: false,
        }
    }

    #[test]
    fn befriending_is_reported_once() {
        let mut people = People::default();
        let friend = |kind| Relationship {
            id: Snowflake(2),
            kind,
            user: None,
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
    fn adopted_dms_are_named_after_recipients() {
        let mut people = People::default();
        let mut dm = Channel {
            kind: ChannelKind::GROUP_DM,
            recipients: vec![user(2, "ana"), user(3, "bia")],
            ..Channel::default()
        };
        people.adopt_dm(&mut dm);
        assert_eq!(dm.name.as_deref(), Some("ana, bia"));
        assert!(dm.recipients.is_empty());
        assert_eq!(people.display_name(Snowflake(3)), Some("bia"));
    }
}
