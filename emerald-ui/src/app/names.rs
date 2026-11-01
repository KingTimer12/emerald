//! Where the pieces beside a name come from: the guild member's name style
//! over the user's, the role icon of the open guild, and the server tag
//! with its badge.

use gpui::{AnyElement, Context, prelude::*};

use super::{Emerald, widgets};
use emerald_discord::model::{NameStyle, PrimaryGuild, Snowflake, role_icon_url};

impl Emerald {
    /// `user`'s display-name style in the open guild: the member's own,
    /// else `fallback` (what the message or user carried).
    pub(super) fn name_style_of<'a>(&'a self, user: Snowflake, fallback: Option<&'a NameStyle>) -> Option<&'a NameStyle> {
        let member = self.spaces.current().info().and_then(|g| g.member(user));
        member.and_then(|m| m.display_name_styles.as_ref()).or(fallback)
    }

    /// The role icon (`role_px` wide) of `user` in the open guild, then
    /// their server tag chip.
    pub(super) fn name_extras(
        &self,
        user: Snowflake,
        tag: Option<&PrimaryGuild>,
        role_px: f32,
        ix: usize,
        _cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut extras = Vec::new();
        if let Some(role) = self.spaces.current().info().and_then(|g| g.icon_role(user)) {
            // Fetched at twice the size, for sharp icons on dense screens.
            let image = role.icon.as_deref().and_then(|hash| self.avatar_image(&role_icon_url(role.id, hash, (role_px * 2.) as u32)));
            let emoji = role.unicode_emoji.as_deref();
            extras.push(widgets::name::role_icon(("role-icon", ix), image, emoji, &role.name, role_px).into_any_element());
        }
        if let Some(tag) = tag.filter(|t| t.shown()) {
            let badge = tag.badge_url(28).and_then(|url| self.avatar_image(&url));
            let text = tag.tag.as_deref().unwrap_or_default();
            extras.push(widgets::name::tag_chip(("tag-chip", ix), text, badge, 14.).into_any_element());
        }
        extras
    }

    /// A DM's name, with the person's server tag after it.
    pub(super) fn dm_name_line(&self, name: gpui::SharedString, user: Option<Snowflake>, ix: usize) -> gpui::Div {
        let tag = user.and_then(|user| self.tag_of(user));
        gpui::div()
            .flex()
            .items_center()
            .gap_1()
            .min_w(crate::zoom::zoomed(0.))
            .line_height(crate::zoom::zoomed(18.))
            .child(gpui::div().truncate().child(name))
            .children(tag.map(|tag| {
                let badge = tag.badge_url(28).and_then(|url| self.avatar_image(&url));
                widgets::name::tag_chip(("dm-tag", ix), tag.tag.as_deref().unwrap_or_default(), badge, 14.)
            }))
    }

    /// The server tag `user` shows, from what the people store knows.
    pub(super) fn tag_of(&self, user: Snowflake) -> Option<&PrimaryGuild> {
        self.people.user(user).and_then(|u| u.primary_guild.as_ref()).filter(|t| t.shown())
    }
}
