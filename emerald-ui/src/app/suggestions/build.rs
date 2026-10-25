//! What to suggest for the word being typed: members, roles, channels or emoji.

use super::*;

impl Emerald {
    pub(super) fn build_suggestions(&self, query: &Query) -> Vec<Suggestion> {
        match query.trigger {
            Trigger::Mention => self.mention_suggestions(&query.text),
            Trigger::Channel => self.channel_suggestions(&query.text),
            Trigger::Emoji => self.emoji_suggestions(&query.text),
            Trigger::Command => self
                .command_suggestions(&query.text)
                .into_iter()
                .map(|(command, app)| Suggestion::Command { command: Box::new(command), app })
                .collect(),
        }
    }

    /// Members of the guild seen so far, or the people in a DM; then the
    /// roles one may ping, and `@everyone` and `@here` when allowed.
    pub(super) fn mention_suggestions(&self, text: &str) -> Vec<Suggestion> {
        let space = self.spaces.current();
        let Some(channel) = self.selected_channel else { return Vec::new() };
        let mut people: Vec<(Snowflake, String, String, Option<u32>)> = match space.info() {
            Some(guild) => guild
                .members()
                .filter_map(|(id, member)| {
                    let user = member.user.as_ref().or_else(|| self.people.user(id))?;
                    let name = member
                        .nick
                        .clone()
                        .or_else(|| user.global_name.clone())
                        .unwrap_or_else(|| user.username.clone());
                    Some((id, name, user.username.clone(), guild.role_color(&member.roles)))
                })
                .collect(),
            None => {
                let recipients = space.channels().iter().find(|c| c.id == channel).map(|c| c.recipients.as_slice());
                recipients
                    .unwrap_or_default()
                    .iter()
                    .chain(self.people.me())
                    .map(|user| {
                        let name = user.global_name.clone().unwrap_or_else(|| user.username.clone());
                        (user.id, name, user.username.clone(), None)
                    })
                    .collect()
            }
        };
        // A steady order for equal matches: by name.
        people.sort_by_cached_key(|(_, name, _, _)| name.to_lowercase());
        let mut found: Vec<Suggestion> = autocomplete::rank(text, people, |(_, name, username, _)| {
            vec![name.as_str(), username.as_str()]
        })
        .into_iter()
        .map(|(id, name, username, color)| Suggestion::Member {
            id,
            name: name.into(),
            username: username.into(),
            color,
        })
        .collect();

        let Some(guild) = space.info() else { return found };
        let everyone = space.permissions(channel) & permissions::MENTION_EVERYONE != 0;
        let mut roles: Vec<_> = guild
            .roles()
            .iter()
            // The @everyone role shares the guild's id and is offered below.
            .filter(|role| role.id != space.id && (role.mentionable || everyone))
            .collect();
        roles.sort_by_key(|role| std::cmp::Reverse(role.position));
        found.extend(
            autocomplete::rank(text, roles, |role| vec![role.name.as_str()])
                .into_iter()
                .take(ROLES)
                .map(|role| Suggestion::Role {
                    id: role.id,
                    name: role.name.clone().into(),
                    color: (role.color != 0).then_some(role.color),
                }),
        );
        if everyone {
            let text = text.to_lowercase();
            found.extend(["everyone", "here"].into_iter().filter(|w| w.starts_with(&text)).map(Suggestion::Everyone));
        }
        found
    }

    /// The guild's channels one can see, threads left out.
    pub(super) fn channel_suggestions(&self, text: &str) -> Vec<Suggestion> {
        let space = self.spaces.current();
        if space.is_dms() {
            return Vec::new();
        }
        let channels = space.channels().iter().filter(|c| {
            !c.kind.is_thread()
                && c.kind != ChannelKind::CATEGORY
                && space.permissions(c.id) & permissions::VIEW_CHANNEL != 0
        });
        autocomplete::rank(text, channels, |c| vec![c.name.as_deref().unwrap_or_default()])
            .into_iter()
            .map(|c| Suggestion::Channel {
                id: c.id,
                name: c.name.clone().unwrap_or_default().into(),
                icon: space.row(c.id).map_or(icons::TEXT, |row| row.icon),
            })
            .collect()
    }

    /// Custom emoji one may use here, then Unicode ones by shortcode.
    pub(super) fn emoji_suggestions(&self, text: &str) -> Vec<Suggestion> {
        let custom = self.custom_emoji().into_iter().flat_map(|(_, emojis)| emojis).filter_map(|emoji| {
            let name = emoji.name.clone()?;
            Some((emoji, name))
        });
        let unicode = emoji::by_category().iter().flat_map(|(_, list)| list).map(|e| {
            let name = emoji::name(e.as_str()).unwrap_or(e.name()).to_owned();
            (Emoji::unicode(e.as_str()), name)
        });
        let text = text.to_lowercase();
        autocomplete::rank(&text, custom.chain(unicode), |(emoji, name)| {
            let mut names = vec![name.as_str()];
            if emoji.id.is_none()
                && let Some(e) = emoji.name.as_deref().and_then(emojis::get)
            {
                names.extend(e.shortcodes());
            }
            names
        })
        .into_iter()
        .map(|(emoji, name)| Suggestion::Emoji { emoji, name: name.into() })
        .collect()
    }
}
