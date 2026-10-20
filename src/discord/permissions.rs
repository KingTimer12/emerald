//! Channel permissions for the current user, computed as Discord documents
//! it: @everyone, then the member's roles, then channel overwrites in the
//! order @everyone, roles, member.

use super::model::{Overwrite, Role, Snowflake, overwrite_kind};

pub const ADMINISTRATOR: u64 = 1 << 3;
pub const ADD_REACTIONS: u64 = 1 << 6;
pub const VIEW_CHANNEL: u64 = 1 << 10;
pub const SEND_MESSAGES: u64 = 1 << 11;
pub const MANAGE_MESSAGES: u64 = 1 << 13;
pub const READ_MESSAGE_HISTORY: u64 = 1 << 16;
/// Split off MANAGE_MESSAGES in 2025: pinning without moderating.
pub const PIN_MESSAGES: u64 = 1 << 51;

/// Everything a guild needs to resolve the current user's permissions.
pub struct GuildAccess<'a> {
    pub guild_id: Snowflake,
    pub owner: Option<Snowflake>,
    pub roles: &'a [Role],
    pub me: Snowflake,
    pub my_roles: &'a [Snowflake],
}

impl GuildAccess<'_> {
    fn base(&self) -> u64 {
        if self.owner == Some(self.me) {
            return u64::MAX;
        }
        // The @everyone role shares the guild's id.
        let mut permissions = self
            .roles
            .iter()
            .filter(|role| role.id == self.guild_id || self.my_roles.contains(&role.id))
            .fold(0, |acc, role| acc | role.permissions);
        if permissions & ADMINISTRATOR != 0 {
            permissions = u64::MAX;
        }
        permissions
    }

    /// Permissions in a channel with `overwrites`. Category overwrites are
    /// not inherited here: Discord copies them into synced children.
    pub fn channel(&self, overwrites: &[Overwrite]) -> u64 {
        let mut permissions = self.base();
        if permissions == u64::MAX {
            return permissions;
        }
        let apply = |permissions: u64, allow: u64, deny: u64| (permissions & !deny) | allow;

        if let Some(everyone) = overwrites
            .iter()
            .find(|o| o.kind == overwrite_kind::ROLE && o.id == self.guild_id)
        {
            permissions = apply(permissions, everyone.allow, everyone.deny);
        }
        let (allow, deny) = overwrites
            .iter()
            .filter(|o| o.kind == overwrite_kind::ROLE && self.my_roles.contains(&o.id))
            .fold((0, 0), |(allow, deny), o| (allow | o.allow, deny | o.deny));
        permissions = apply(permissions, allow, deny);
        if let Some(member) = overwrites
            .iter()
            .find(|o| o.kind == overwrite_kind::MEMBER && o.id == self.me)
        {
            permissions = apply(permissions, member.allow, member.deny);
        }
        permissions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GUILD: Snowflake = Snowflake(1);
    const ME: Snowflake = Snowflake(2);
    const MOD: Snowflake = Snowflake(3);

    fn role(id: Snowflake, permissions: u64) -> Role {
        Role {
            id,
            name: String::new(),
            color: 0,
            position: 0,
            permissions,
        }
    }

    fn overwrite(id: Snowflake, kind: u8, allow: u64, deny: u64) -> Overwrite {
        Overwrite { id, kind, allow, deny }
    }

    fn access<'a>(roles: &'a [Role], mine: &'a [Snowflake]) -> GuildAccess<'a> {
        GuildAccess {
            guild_id: GUILD,
            owner: None,
            roles,
            me: ME,
            my_roles: mine,
        }
    }

    #[test]
    fn everyone_deny_hides_channel_unless_role_allows() {
        let roles = [role(GUILD, VIEW_CHANNEL | SEND_MESSAGES), role(MOD, 0)];
        let overwrites = [
            overwrite(GUILD, overwrite_kind::ROLE, 0, VIEW_CHANNEL),
            overwrite(MOD, overwrite_kind::ROLE, VIEW_CHANNEL, 0),
        ];
        assert_eq!(access(&roles, &[]).channel(&overwrites) & VIEW_CHANNEL, 0);
        assert_ne!(access(&roles, &[MOD]).channel(&overwrites) & VIEW_CHANNEL, 0);
    }

    #[test]
    fn member_overwrite_wins_over_roles() {
        let roles = [role(GUILD, VIEW_CHANNEL | SEND_MESSAGES)];
        let overwrites = [overwrite(ME, overwrite_kind::MEMBER, 0, SEND_MESSAGES)];
        let permissions = access(&roles, &[]).channel(&overwrites);
        assert_ne!(permissions & VIEW_CHANNEL, 0);
        assert_eq!(permissions & SEND_MESSAGES, 0);
    }

    #[test]
    fn administrator_and_owner_bypass_overwrites() {
        let roles = [role(GUILD, 0), role(MOD, ADMINISTRATOR)];
        let deny_all = [overwrite(GUILD, overwrite_kind::ROLE, 0, u64::MAX)];
        assert_eq!(access(&roles, &[MOD]).channel(&deny_all), u64::MAX);
        let owner = GuildAccess {
            owner: Some(ME),
            ..access(&roles, &[])
        };
        assert_eq!(owner.channel(&deny_all), u64::MAX);
    }
}
