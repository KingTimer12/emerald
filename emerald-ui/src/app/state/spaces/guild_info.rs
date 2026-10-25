//! What a guild space knows to decide permissions, names and role colors.

use super::*;

/// Guild data needed for permissions, names and role colors.
pub struct GuildInfo {
    /// The current user, whose permissions are computed.
    pub(super) me: Snowflake,
    pub(super) owner: Option<Snowflake>,
    pub(super) roles: Vec<Role>,
    /// The current user's roles here.
    pub(super) my_roles: Vec<Snowflake>,
    /// Nicknames and roles of message authors seen so far.
    pub(super) members: HashMap<Snowflake, Member>,
    /// Authors already asked for with op 8, so each is requested once.
    pub(super) requested: HashSet<Snowflake>,
    /// The current user's permissions per channel.
    pub(super) permissions: HashMap<Snowflake, u64>,
    pub(super) emojis: Vec<GuildEmoji>,
    pub(super) stickers: Vec<StickerItem>,
    /// Upload size the guild's boosts unlock.
    pub(super) upload_limit: Option<u64>,
}

impl GuildInfo {
    /// A role's guild-wide permissions; 0 for an unknown role.
    pub fn role_permissions(&self, role: Snowflake) -> u64 {
        self.roles.iter().find(|r| r.id == role).map_or(0, |r| r.permissions)
    }

    pub fn upload_limit(&self) -> Option<u64> {
        self.upload_limit
    }

    /// Custom emoji uploaded here.
    pub fn emojis(&self) -> &[GuildEmoji] {
        &self.emojis
    }

    pub fn set_emojis(&mut self, emojis: Vec<GuildEmoji>) {
        self.emojis = emojis;
    }

    /// Stickers uploaded here.
    pub fn stickers(&self) -> &[StickerItem] {
        &self.stickers
    }

    pub fn set_stickers(&mut self, stickers: Vec<StickerItem>) {
        self.stickers = stickers;
    }

    /// Display name and role color of a member, when known.
    pub fn member_style(&self, user: Snowflake) -> (Option<&str>, Option<u32>) {
        let Some(member) = self.members.get(&user) else { return (None, None) };
        (member.nick.as_deref(), self.role_color(&member.roles))
    }

    /// Color of the highest colored role among `roles`.
    pub fn role_color(&self, roles: &[Snowflake]) -> Option<u32> {
        self.roles
            .iter()
            .filter(|role| role.color != 0 && roles.contains(&role.id))
            .max_by_key(|role| role.position)
            .map(|role| role.color)
    }

    pub fn roles(&self) -> &[Role] {
        &self.roles
    }

    /// Members seen so far: message authors, the member list and searches.
    pub fn members(&self) -> impl Iterator<Item = (Snowflake, &Member)> {
        self.members.iter().map(|(id, member)| (*id, member))
    }

    pub fn role(&self, id: Snowflake) -> Option<&Role> {
        self.roles.iter().find(|r| r.id == id)
    }

    /// Nickname and roles of a member seen before.
    pub fn member(&self, user: Snowflake) -> Option<&Member> {
        self.members.get(&user)
    }

    /// The known roles among `ids`, highest first, as Discord lists them.
    pub fn roles_of(&self, ids: &[Snowflake]) -> Vec<&Role> {
        let mut roles: Vec<&Role> = self.roles.iter().filter(|r| ids.contains(&r.id)).collect();
        roles.sort_by_key(|r| std::cmp::Reverse(r.position));
        roles
    }

    pub fn add_member(&mut self, user: Snowflake, member: Member) {
        self.members.insert(user, member);
    }

    /// The current user's roles here, which decide who a message pings.
    pub fn my_roles(&self) -> &[Snowflake] {
        &self.my_roles
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
