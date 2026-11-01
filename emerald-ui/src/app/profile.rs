//! User profile beside 1:1 DMs and the "View Full Profile" modal, both
//! fed by `GET /users/{id}/profile`.

use crate::zoom::zoomed;

mod modal;
mod own;
mod panel;
mod popout;
mod social;
mod board;

pub(super) use own::OwnPopover;
pub(super) use popout::Popout;
pub(super) use social::ProfileTab;

use std::rc::Rc;

use gpui::{Context, Div, FontWeight, ImageSource, MouseButton, ObjectFit, SharedString, Window, div, img, prelude::*, rgb};

use super::super::{theme, time};
use super::state::profile_theme::{ProfileTheme, profile_theme};
use super::widgets::{self, ButtonKind};
use super::{Emerald, MAX_PROFILE_EFFECTS, Overlay};
use emerald_discord::Command;
use emerald_discord::model::{
    NameStyle, PROFILE_EFFECT, PrimaryGuild, Profile, ProfileEffect, Snowflake, User, banner_url, guild_banner_url,
};

const PANEL_WIDTH: f32 = 320.;

/// What both profile views show, resolved from the user and profile caches.
struct Card {
    name: SharedString,
    username: SharedString,
    banner: u32,
    /// The banner image, drawn over the color once downloaded.
    banner_url: Option<String>,
    pronouns: Option<String>,
    bio: Option<String>,
    /// Badge descriptions with their icon URL.
    badges: Vec<(SharedString, String)>,
    /// The Nitro profile effect, drawn over the whole card.
    effect: Option<Rc<ProfileEffect>>,
    /// Connected accounts as (service, account name).
    accounts: Vec<(String, String)>,
    /// Their roles in the guild, highest first, as (name, color).
    roles: Vec<(String, u32)>,
    /// When they joined the guild, in Unix seconds.
    joined: Option<i64>,
    /// The card's colors when they set theme colors.
    theme: Option<ProfileTheme>,
    tag: Option<PrimaryGuild>,
    name_style: Option<NameStyle>,
    /// Friends in common, as asked for by the popout, and how many
    /// friends and servers are shared when known.
    mutual_friends: Vec<User>,
    mutual_friend_count: Option<usize>,
    mutual_server_count: Option<usize>,
}

impl Emerald {
    /// The card of `user`, as members of `guild` see them when given.
    fn card(&self, user: Snowflake, guild: Option<Snowflake>) -> Card {
        let known = self.people.user(user);
        let profile = self.profiles.get(&(user, guild));
        // Global badges and accounts come with the guild-less profile.
        let global_profile = self.profiles.get(&(user, None));
        let info = guild.and_then(|id| self.spaces.guild(id));
        let membership = profile.and_then(|p| p.guild_member.as_ref());
        let seen = info.and_then(|info| info.member(user));
        // Guild-only bio and pronouns win over the global ones when set.
        let local = profile.and_then(|p| p.guild_member_profile.as_ref());
        let global = global_profile.or(profile).map(|p| &p.user_profile);
        let pronouns = local
            .and_then(|d| d.pronouns.clone())
            .filter(|p| !p.is_empty())
            .or_else(|| global.and_then(|d| d.pronouns.clone()).filter(|p| !p.is_empty()));
        let bio = local
            .and_then(|d| d.bio.clone())
            .filter(|b| !b.trim().is_empty())
            .or_else(|| global.and_then(|d| d.bio.clone()).filter(|b| !b.trim().is_empty()));
        let nick = membership
            .and_then(|m| m.nick.clone())
            .or_else(|| seen.and_then(|m| m.nick.clone()));
        let role_ids = membership.map(|m| m.roles.as_slice()).or(seen.map(|m| m.roles.as_slice()));
        let roles = info
            .zip(role_ids)
            .map(|(info, ids)| {
                info.roles_of(ids)
                    .into_iter()
                    .map(|r| (r.name.clone(), r.color))
                    .collect()
            })
            .unwrap_or_default();
        let banner_url = guild
            .zip(local.and_then(|d| d.banner.as_deref()))
            .map(|(guild, hash)| guild_banner_url(guild, user, hash))
            .or_else(|| global_profile.or(profile)?.user.banner.as_deref().map(|hash| banner_url(user, hash)));
        let effect = profile
            .and_then(profile_effect)
            .or_else(|| global_profile.and_then(profile_effect))
            .and_then(|sku| self.profile_effects.get(sku).cloned().flatten());
        let theme_colors = local.and_then(|d| d.theme_colors).or_else(|| global.and_then(|d| d.theme_colors));
        let asked = profile;
        let mutual_friends = asked.and_then(|p| p.mutual_friends.clone()).unwrap_or_default();
        let mutual_friend_count = asked
            .and_then(|p| p.mutual_friends.as_ref().map(Vec::len).or(p.mutual_friends_count.map(|n| n as usize)));
        let mutual_server_count = asked.and_then(|p| p.mutual_guilds.as_ref().map(Vec::len));
        let profile = global_profile.or(profile);
        let profile_user = profile.and_then(|p| p.user.user());
        let tag = known
            .and_then(|u| u.primary_guild.clone())
            .or_else(|| profile_user.as_ref().and_then(|u| u.primary_guild.clone()))
            .filter(PrimaryGuild::shown);
        let name_style = seen
            .and_then(|m| m.display_name_styles.clone())
            .or_else(|| known.and_then(|u| u.display_name_styles.clone()))
            .or_else(|| profile_user.and_then(|u| u.display_name_styles));
        Card {
            theme: theme_colors.map(|[primary, accent]| profile_theme(primary, accent)),
            tag,
            name_style,
            mutual_friends,
            mutual_friend_count,
            mutual_server_count,
            effect,
            banner_url,
            name: nick
                .or_else(|| known.map(|u| u.display_name().to_owned()))
                .unwrap_or_else(|| user.to_string())
                .into(),
            roles,
            joined: membership.and_then(|m| m.joined_secs()),
            username: known.map(|u| u.username.clone()).unwrap_or_default().into(),
            banner: profile
                .and_then(|p| {
                    p.user
                        .banner_color
                        .as_deref()
                        .and_then(|hex| u32::from_str_radix(hex.trim_start_matches('#'), 16).ok())
                        .or(p.user.accent_color)
                })
                // Without a banner color Discord paints a neutral grey.
                .unwrap_or(theme::interactive_muted()),
            pronouns,
            bio,
            badges: profile.map_or_else(Vec::new, |p| {
                p.badges
                    .iter()
                    .map(|b| (b.description.clone().into(), b.icon_url()))
                    .collect()
            }),
            accounts: profile.map_or_else(Vec::new, |p| {
                p.connected_accounts
                    .iter()
                    .map(|a| (a.kind.clone(), a.name.clone()))
                    .collect()
            }),
        }
    }

    /// The banner image when there is one and it has arrived, else its color.
    fn banner(&self, card: &Card, height: f32) -> Div {
        let banner = div().w_full().h(zoomed(height)).flex_none().overflow_hidden().bg(rgb(card.banner));
        match card.banner_url.as_deref().and_then(|url| self.avatar_image(url)) {
            Some(image) => banner.child(img(image).w_full().h(zoomed(height)).object_fit(gpui::ObjectFit::Cover)),
            None => banner,
        }
    }

    /// The custom status and what `user` is doing, as far as Emerald sees:
    /// friends' presences only.
    fn activities(&self, user: Snowflake) -> impl Iterator<Item = Div> + '_ {
        let activities = self.people.activities(user);
        let custom = activities
            .iter()
            .find(|a| a.kind == emerald_discord::model::activity_kind::CUSTOM)
            .and_then(|a| a.custom_text())
            .map(|status| div().text_size(zoomed(14.)).text_color(rgb(theme::text_default())).child(status));
        let others = activities.iter().filter_map(|activity| {
            let headline = super::activity::headline(activity)?;
            let lines = [activity.details.clone(), activity.state.clone()].into_iter().flatten();
            Some(section(
                t!("profile.activity"),
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(zoomed(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(theme::text_strong()))
                            .child(headline),
                    )
                    .children(lines.map(|line| div().text_size(zoomed(13.)).text_color(rgb(theme::text_muted())).child(line))),
            ))
        });
        custom.into_iter().chain(others)
    }

    fn avatar_ring(&self, user: Snowflake, card: &Card, size: f32, ring: f32) -> Div {
        let avatar = widgets::avatar(
            &card.name,
            user,
            self.profile_avatar(user),
            size,
            // Emerald does not get its own presence back; the status picked is it.
            Some((
                if self.people.is_me(user) { self.own_status().unwrap_or("offline") } else { self.people.presence(user) },
                theme::base_lowest(),
            )),
        );
        div()
            .p(zoomed(ring))
            .rounded_full()
            .bg(rgb(theme::base_lowest()))
            .child(widgets::decorated(avatar, self.decoration_of(user, size, true), size))
    }

    /// The profile effect over the whole card, `width` pixels wide: its
    /// layers on their clock from when the profile opened while the window
    /// animates, else its still.
    fn effect(&self, card: &Card, width: f32) -> Option<Div> {
        let effect = card.effect.as_ref()?;
        let cover = |image: ImageSource| {
            img(image.clone())
                .id(widgets::frames_id("effect", &image))
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .object_fit(ObjectFit::Cover)
        };
        let layer = div().absolute().top_0().left_0().size_full();
        if !self.animates() || effect.layers.is_empty() {
            let still = self.avatar_image(effect.still.as_deref()?)?;
            return Some(layer.child(cover(ImageSource::Image(still))));
        }
        let elapsed = self.profile_opened.elapsed().as_millis() as u64;
        // Layers fill the card's width, twice that for Retina, and as tall
        // as twice that again.
        let (pixels_w, pixels_h) = ((width * 2.) as u32, (width * 4.) as u32);
        let shown = effect.layers.iter().filter_map(|layer| {
            if layer.changes_after(elapsed) {
                // A layer still to come or go needs frames on its clock.
                self.animating.set(true);
            }
            let frames = layer.shown(elapsed).then(|| self.images.animated(&layer.src, pixels_w, pixels_h, &*self.net))??;
            Some(cover(ImageSource::Render(frames)))
        });
        Some(layer.children(shown))
    }

    /// Asks once for the still of the profile effect `profile` shows, as
    /// the web client fetches its product right after the profile.
    pub(super) fn ask_profile_effect(&mut self, profile: &Profile) {
        let Some(sku) = profile_effect(profile) else { return };
        if self.profile_effects.contains_key(sku) {
            return;
        }
        if self.profile_effects.len() >= MAX_PROFILE_EFFECTS {
            self.profile_effects.clear();
        }
        self.profile_effects.insert(sku.to_owned(), None);
        self.net.post(Command::FetchProfileEffect { sku: sku.to_owned() });
    }

    pub(super) fn open_profile(
        &mut self,
        user: Snowflake,
        guild: Option<Snowflake>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.profile(user, guild, emerald_discord::ProfileKind::Modal);
        // Badges and connected accounts only come with the global profile.
        if guild.is_some() {
            self.profile(user, None, emerald_discord::ProfileKind::Panel);
        }
        self.profile_tab = ProfileTab::About;
        self.profile_opened = std::time::Instant::now();
        if !self.people.is_me(user) {
            self.load_note(user, cx);
            // Without a Board the modal opens on Mutual Friends.
            if !self.mutual_friends.contains_key(&user) {
                self.net.post(Command::MutualFriends { user });
            }
        }
        self.overlay = Some(Overlay::Profile { user, guild });
        window.focus(&self.focus_handle);
        cx.notify();
    }
}

/// The SKU of the profile effect shown: the server one first.
fn profile_effect(profile: &Profile) -> Option<&str> {
    let now = time::now_secs();
    let guild = profile.guild_member_profile.as_ref().map_or(&[][..], |p| p.collectibles.as_slice());
    guild
        .iter()
        .chain(&profile.user_profile.collectibles)
        .find(|item| item.kind == PROFILE_EFFECT && item.active(now))
        .map(|item| item.sku_id.as_str())
}

/// Discord's join date, plus the guild's when the card is a member's.
fn member_since(user: Snowflake, card: &Card, guild: Option<SharedString>) -> Div {
    let discord = time::short_date(user.unix_secs());
    match (guild, card.joined) {
        (Some(guild), Some(joined)) => div()
            .flex()
            .flex_col()
            .child(text(format!("Discord: {discord}")))
            .child(text(format!("{guild}: {}", time::short_date(joined)))),
        _ => text(discord),
    }
}

fn section(title: &'static str, body: impl IntoElement) -> Div {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(zoomed(14.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(theme::text_strong()))
                .child(title),
        )
        .child(body)
}

fn text(text: String) -> Div {
    div()
        .text_size(zoomed(14.))
        .line_height(zoomed(18.))
        .text_color(rgb(theme::text_default()))
        .child(text)
}
