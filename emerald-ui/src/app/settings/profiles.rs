//! Profiles: the user's profile everywhere, or as one server sees it, with
//! a preview of the card others open. Only what changed is sent.

use std::sync::Arc;

use gpui::{ImageFormat, PathPromptOptions, img};

use super::super::Still;

use super::*;
use emerald_discord::model::{banner_url, guild_banner_url};
use emerald_discord::{Picture, ProfileEdit};
use crate::zoom::zoomed;

/// Discord's limits on each field.
const MAX_NAME: usize = 32;
const MAX_PRONOUNS: usize = 40;
const MAX_BIO: usize = 190;
/// Largest image sent as an avatar or banner.
const PICTURE_LIMIT: usize = 8 * 1024 * 1024;

/// The banner colors offered, as in the web client's color picker presets.
const ACCENT_COLORS: [u32; 10] = [
    0x5865f2, 0x3ba55c, 0xfaa61a, 0xed4245, 0xeb459e, 0x9b59b6, 0x1abc9c, 0x3498db, 0x99aab5, 0x2c2f33,
];

pub(in super::super) struct ProfileForm {
    /// `None` edits the profile everywhere.
    pub guild: Option<Snowflake>,
    /// The display name, or the server nickname.
    name: Entity<TextInput>,
    pronouns: Entity<TextInput>,
    bio: Entity<TextInput>,
    /// What the three fields held when filled, to send only changes.
    original: [String; 3],
    /// Filled from a profile that had arrived; until then it fills again
    /// when it does.
    filled: bool,
    avatar: Option<Option<Picture>>,
    avatar_preview: Option<Still>,
    banner: Option<Option<Picture>>,
    banner_preview: Option<Still>,
    accent_color: Option<Option<u32>>,
    servers_open: bool,
}

#[derive(Clone, Copy)]
enum Slot {
    Avatar,
    Banner,
}

impl ProfileForm {
    fn new(guild: Option<Snowflake>, cx: &mut Context<Emerald>) -> Self {
        let name_hint = if guild.is_some() { t!("profiles.nickname") } else { t!("settings.display_name_placeholder") };
        Self {
            guild,
            name: cx.new(|cx| TextInput::new(name_hint, cx).keep_on_submit()),
            pronouns: cx.new(|cx| TextInput::new(t!("profiles.pronouns_placeholder"), cx).keep_on_submit()),
            bio: cx.new(|cx| TextInput::new(t!("profiles.bio_placeholder"), cx).keep_on_submit().multiline(6)),
            original: Default::default(),
            filled: false,
            avatar: None,
            avatar_preview: None,
            banner: None,
            banner_preview: None,
            accent_color: None,
            servers_open: false,
        }
    }

    fn texts(&self, cx: &App) -> [String; 3] {
        [&self.name, &self.pronouns, &self.bio].map(|input| input.read(cx).text().trim().to_owned())
    }
}

impl Emerald {
    fn has_nitro(&self) -> bool {
        self.people.me().and_then(|me| me.premium_type).unwrap_or(0) > 0
    }

    /// Opens the form on the profile everywhere, or on `guild`'s.
    pub(super) fn open_profile_form(&mut self, guild: Option<Snowflake>, cx: &mut Context<Self>) {
        let keep = self
            .settings_view()
            .and_then(|view| view.profile.as_ref())
            .is_some_and(|form| form.guild == guild);
        if !keep {
            let form = ProfileForm::new(guild, cx);
            if let Some(view) = self.settings_view() {
                view.profile = Some(form);
                view.feedback = None;
            }
        }
        let me = self.people.my_id();
        self.profile(me, guild, false);
        self.fill_profile_form(cx);
    }

    /// Puts the current values in the form, unless it was filled from an
    /// arrived profile already.
    pub(in super::super) fn fill_profile_form(&mut self, cx: &mut Context<Self>) {
        let Some(guild) = self.settings_view().and_then(|v| v.profile.as_ref()).filter(|f| !f.filled).map(|f| f.guild)
        else {
            return;
        };
        let me = self.people.my_id();
        let profile = self.profiles.get(&(me, guild));
        let details = match guild {
            Some(_) => profile.and_then(|p| p.guild_member_profile.as_ref()),
            None => profile.map(|p| &p.user_profile),
        };
        let name = match guild {
            Some(guild) => profile
                .and_then(|p| p.guild_member.as_ref())
                .and_then(|m| m.nick.clone())
                .or_else(|| self.spaces.guild(guild)?.member(me)?.nick.clone()),
            None => self.people.me().and_then(|me| me.global_name.clone()),
        };
        let texts = [
            name.unwrap_or_default(),
            details.and_then(|d| d.pronouns.clone()).unwrap_or_default(),
            details.and_then(|d| d.bio.clone()).unwrap_or_default(),
        ];
        let filled = profile.is_some();
        let Some(form) = self.settings_view().and_then(|v| v.profile.as_mut()) else { return };
        for (input, text) in [&form.name, &form.pronouns, &form.bio].into_iter().zip(&texts) {
            input.update(cx, |input, cx| input.set_text(text.clone(), cx));
        }
        form.original = texts;
        form.filled = filled;
    }

    fn pick_profile_picture(&mut self, slot: Slot, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else { return };
            let Some(path) = paths.into_iter().next() else { return };
            let Some((mime, format)) = picture_type(&path) else { return };
            let read = cx.background_spawn(async move { std::fs::read(path).ok() }).await;
            this.update(cx, |this, cx| {
                let Some(bytes) = read else { return };
                let Some(view) = this.settings_view() else { return };
                if bytes.len() > PICTURE_LIMIT {
                    view.feedback = Some(Feedback::from_result(Err(t!("errors.image_too_large").into()), ""));
                    cx.notify();
                    return;
                }
                let Some(form) = view.profile.as_mut() else { return };
                let preview = Some(Still::new(format, bytes.clone()));
                let picture = Some(Some((mime, Arc::<[u8]>::from(bytes))));
                match slot {
                    Slot::Avatar => (form.avatar, form.avatar_preview) = (picture, preview),
                    Slot::Banner => (form.banner, form.banner_preview) = (picture, preview),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn remove_profile_picture(&mut self, slot: Slot, cx: &mut Context<Self>) {
        if let Some(form) = self.settings_view().and_then(|v| v.profile.as_mut()) {
            match slot {
                Slot::Avatar => (form.avatar, form.avatar_preview) = (Some(None), None),
                Slot::Banner => (form.banner, form.banner_preview) = (Some(None), None),
            }
        }
        cx.notify();
    }

    fn save_profile(&mut self, cx: &mut Context<Self>) {
        let Some(view) = self.settings_view() else { return };
        let Some(form) = view.profile.as_ref() else { return };
        let [name, pronouns, bio] = form.texts(cx);
        let too_long = [(&name, MAX_NAME), (&pronouns, MAX_PRONOUNS), (&bio, MAX_BIO)]
            .into_iter()
            .find(|(text, max)| text.chars().count() > *max);
        if let Some((_, max)) = too_long {
            view.feedback = Some(Feedback::from_result(Err(t!("profiles.too_long", max = max)), ""));
            cx.notify();
            return;
        }
        let changed = |ix: usize, text: &String| (*text != form.original[ix]).then(|| text.clone());
        let edit = ProfileEdit {
            name: changed(0, &name).map(|n| (!n.is_empty()).then_some(n)),
            avatar: form.avatar.clone(),
            banner: form.banner.clone(),
            bio: changed(2, &bio),
            pronouns: changed(1, &pronouns),
            accent_color: form.accent_color,
        };
        let guild = form.guild;
        view.feedback = None;
        if edit.touches_user() || edit.touches_profile() {
            self.run(Action::UpdateProfile, Command::EditProfile { guild, edit }, cx);
        }
    }

    /// After a save: what was sent is now the profile.
    pub(in super::super) fn profile_saved(&mut self, cx: &mut Context<Self>) {
        let Some(form) = self.settings_view().and_then(|v| v.profile.as_mut()) else { return };
        form.original = form.texts(cx);
        form.avatar = None;
        form.banner = None;
        form.accent_color = None;
    }

    pub(super) fn render_profiles(&self, view: &SettingsView, cx: &mut Context<Self>) -> AnyElement {
        let Some(form) = view.profile.as_ref() else { return page_title(t!("settings.profiles")).into_any_element() };
        let nitro = self.has_nitro();
        let guild = form.guild;
        let busy = self.busy.contains(&Action::UpdateProfile);
        let tab = |id: &'static str, label: &'static str, selected: bool| {
            div()
                .id(id)
                .pb_2()
                .border_b_2()
                .border_color(if selected { rgb(theme::brand()).into() } else { gpui::transparent_black() })
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .text_color(rgb(if selected { theme::text_strong() } else { theme::interactive() }))
                .child(label)
        };
        let field = |label: &'static str, input: &Entity<TextInput>, locked: bool| {
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(widgets::label(label))
                .child(if locked { hint(t!("profiles.nitro_only")) } else { div().child(input.clone()) })
        };
        let picture_buttons = |slot: Slot, change: &'static str, remove: &'static str, cx: &mut Context<Self>| {
            let ids = match slot {
                Slot::Avatar => ("change-avatar", "remove-avatar"),
                Slot::Banner => ("change-banner", "remove-banner"),
            };
            div()
                .flex()
                .gap_3()
                .child(
                    widgets::button(ids.0, change, ButtonKind::Brand)
                        .on_click(cx.listener(move |this, _, _, cx| this.pick_profile_picture(slot, cx))),
                )
                .child(
                    widgets::button(ids.1, remove, ButtonKind::Secondary)
                        .on_click(cx.listener(move |this, _, _, cx| this.remove_profile_picture(slot, cx))),
                )
        };
        let avatar_label = if guild.is_some() { t!("profiles.server_avatar") } else { t!("profiles.avatar") };

        let mut fields = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(zoomed(0.))
            .gap_6()
            .child(field(
                if guild.is_some() { t!("profiles.nickname") } else { t!("settings.display_name_label") },
                &form.name,
                false,
            ))
            .child(field(t!("profiles.pronouns"), &form.pronouns, guild.is_some() && !nitro))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(widgets::label(avatar_label))
                    .child(if guild.is_some() && !nitro {
                        hint(t!("profiles.nitro_only"))
                    } else {
                        picture_buttons(Slot::Avatar, t!("profiles.change_avatar"), t!("profiles.remove_avatar"), cx)
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(widgets::label(t!("profiles.banner")))
                    .child(if nitro {
                        picture_buttons(Slot::Banner, t!("profiles.change_banner"), t!("profiles.remove_banner"), cx)
                    } else {
                        hint(t!("profiles.nitro_only"))
                    }),
            );
        if guild.is_none() {
            fields = fields.child(self.render_accent_colors(form, cx));
        }
        fields = fields
            .child(field(t!("profiles.bio"), &form.bio, guild.is_some() && !nitro))
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        widgets::button(
                            "save-profile",
                            if busy { t!("common.saving") } else { t!("common.save") },
                            ButtonKind::Brand,
                        )
                        .when(busy, |b| b.opacity(0.5))
                        .on_click(cx.listener(|this, _, _, cx| this.save_profile(cx))),
                    )
                    .child(
                        widgets::button("reset-profile", t!("profiles.reset"), ButtonKind::Secondary).on_click(
                            cx.listener(move |this, _, _, cx| {
                                if let Some(view) = this.settings_view() {
                                    view.profile = None;
                                }
                                this.open_profile_form(guild, cx);
                                cx.notify();
                            }),
                        ),
                    ),
            )
            .children(view.feedback.as_ref().map(widgets::feedback));

        div()
            .flex()
            .flex_col()
            .child(page_title(t!("settings.profiles")))
            .child(
                div()
                    .mb_6()
                    .flex()
                    .gap_6()
                    .border_b_1()
                    .border_color(theme::border())
                    .child(tab("tab-user-profile", t!("profiles.user_tab"), guild.is_none()).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.open_profile_form(None, cx);
                            cx.notify();
                        },
                    )))
                    .child(tab("tab-server-profile", t!("profiles.server_tab"), guild.is_some()).on_click(
                        cx.listener(|this, _, _, cx| {
                            let first = this.spaces.iter().find(|s| !s.is_dms()).map(|s| s.id);
                            let current = this.settings_view().and_then(|v| v.profile.as_ref()).and_then(|f| f.guild);
                            if let Some(guild) = current.or(first) {
                                this.open_profile_form(Some(guild), cx);
                            }
                            cx.notify();
                        }),
                    )),
            )
            .when(guild.is_some(), |page| page.child(self.render_server_choice(form, cx)))
            .child(div().flex().items_start().gap_8().child(fields).child(self.render_profile_preview(form, cx)))
            .into_any_element()
    }

    fn render_accent_colors(&self, form: &ProfileForm, cx: &mut Context<Self>) -> gpui::Div {
        let me = self.people.my_id();
        let saved = self.profiles.get(&(me, None)).and_then(|p| p.user.accent_color);
        let current = form.accent_color.unwrap_or(saved);
        let swatch = |ix: usize, color: Option<u32>| {
            let selected = current == color;
            div()
                .id(("accent", ix))
                .size(zoomed(32.))
                .rounded(theme::radius::control())
                .cursor_pointer()
                .border_2()
                .border_color(if selected { rgb(theme::text_strong()) } else { theme::border() })
                .bg(rgb(color.unwrap_or(theme::base_lowest())))
                .when(color.is_none(), |s| {
                    s.flex().items_center().justify_center().text_color(rgb(theme::text_muted())).child("∅")
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(form) = this.settings_view().and_then(|v| v.profile.as_mut()) {
                        form.accent_color = Some(color);
                    }
                    cx.notify();
                }))
        };
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(widgets::label(t!("profiles.accent_color")))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .child(swatch(0, None))
                    .children(ACCENT_COLORS.iter().enumerate().map(|(ix, color)| swatch(ix + 1, Some(*color)))),
            )
    }

    fn render_server_choice(&self, form: &ProfileForm, cx: &mut Context<Self>) -> gpui::Div {
        let current = form.guild.and_then(|g| self.spaces.iter().find(|s| s.id == g)).map(|s| s.name.clone());
        let open = form.servers_open;
        div()
            .mb_6()
            .flex()
            .flex_col()
            .gap_2()
            .child(widgets::label(t!("profiles.choose_server")))
            .child(
                div()
                    .id("server-choice")
                    .px_3()
                    .py_2()
                    .rounded(theme::radius::control())
                    .bg(rgb(theme::base_lowest()))
                    .cursor_pointer()
                    .flex()
                    .justify_between()
                    .text_color(rgb(theme::text_default()))
                    .child(current.unwrap_or_default())
                    .child(if open { "▴" } else { "▾" })
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(form) = this.settings_view().and_then(|v| v.profile.as_mut()) {
                            form.servers_open = !form.servers_open;
                        }
                        cx.notify();
                    })),
            )
            .when(open, |col| {
                col.child(
                    div()
                        .id("server-choices")
                        .max_h(zoomed(240.))
                        .overflow_y_scroll()
                        .rounded(theme::radius::control())
                        .bg(rgb(theme::base_lowest()))
                        .children(self.spaces.iter().filter(|s| !s.is_dms()).enumerate().map(|(ix, space)| {
                            let guild = space.id;
                            div()
                                .id(("server-choice-item", ix))
                                .px_3()
                                .py_2()
                                .cursor_pointer()
                                .text_color(rgb(theme::text_default()))
                                .hover(|s| s.bg(theme::hover()))
                                .child(space.name.clone())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.open_profile_form(Some(guild), cx);
                                    cx.notify();
                                }))
                        })),
                )
            })
    }

    /// The card as others will see it, with the picked images and color.
    fn render_profile_preview(&self, form: &ProfileForm, cx: &App) -> gpui::Div {
        let me = self.people.me();
        let id = self.people.my_id();
        let profile = self.profiles.get(&(id, form.guild));
        let global = self.profiles.get(&(id, None));
        let [name, pronouns, bio] = form.texts(cx);
        let shown_name: SharedString = if name.is_empty() {
            me.map_or_else(String::new, |me| match form.guild {
                Some(_) => me.display_name().to_owned(),
                None => me.username.clone(),
            })
            .into()
        } else {
            name.into()
        };
        let banner_hash = match form.guild {
            Some(guild) => profile
                .and_then(|p| p.guild_member_profile.as_ref())
                .and_then(|d| d.banner.as_deref())
                .map(|hash| guild_banner_url(guild, id, hash)),
            None => global.and_then(|p| p.user.banner.as_deref()).map(|hash| banner_url(id, hash)),
        };
        let banner_image = match &form.banner {
            Some(Some(_)) => form.banner_preview.as_ref().map(Still::image),
            Some(None) => None,
            None => banner_hash.and_then(|url| self.avatar_image(&url)),
        };
        let accent = form
            .accent_color
            .unwrap_or_else(|| global.and_then(|p| p.user.accent_color))
            .unwrap_or(theme::brand());
        let avatar = match &form.avatar {
            Some(Some(_)) => form.avatar_preview.as_ref().map(Still::image),
            Some(None) => self.avatar_image(&emerald_discord::model::avatar_url(id, None)),
            None => me.and_then(|me| self.avatar_image(&me.avatar_url())),
        };

        div()
            .w(zoomed(300.))
            .flex_none()
            .rounded(theme::radius::card())
            .overflow_hidden()
            .bg(rgb(theme::base_lowest()))
            .child(match banner_image {
                Some(image) => div().h(zoomed(105.)).overflow_hidden().child(img(image).w_full().h(zoomed(105.)).object_fit(gpui::ObjectFit::Cover)),
                None => div().h(zoomed(105.)).bg(rgb(accent)),
            })
            .child(
                div().px_4().child(
                    div()
                        .mt(zoomed(-40.))
                        .p(zoomed(6.))
                        .w(zoomed(92.))
                        .rounded_full()
                        .bg(rgb(theme::base_lowest()))
                        .child(widgets::avatar(&shown_name, id, avatar, 80., None)),
                ),
            )
            .child(
                div()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_size(zoomed(20.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(theme::text_strong()))
                            .child(shown_name),
                    )
                    .children(me.map(|me| hint(me.username.clone())))
                    .when(!pronouns.is_empty(), |col| col.child(hint(pronouns)))
                    .when(!bio.is_empty(), |col| col.child(div().mt_3().text_size(zoomed(14.)).text_color(rgb(theme::text_default())).child(bio))),
            )
    }
}

/// The media type Discord takes and the format to preview, by extension.
fn picture_type(path: &std::path::Path) -> Option<(&'static str, ImageFormat)> {
    match path.extension()?.to_str()?.to_lowercase().as_str() {
        "png" => Some(("image/png", ImageFormat::Png)),
        "jpg" | "jpeg" => Some(("image/jpeg", ImageFormat::Jpeg)),
        "gif" => Some(("image/gif", ImageFormat::Gif)),
        "webp" => Some(("image/webp", ImageFormat::Webp)),
        _ => None,
    }
}
