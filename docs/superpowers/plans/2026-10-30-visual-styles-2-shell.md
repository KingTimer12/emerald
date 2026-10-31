# Visual styles, plan 2 of 6: the shell (sidebars, Home, headers, member list, user panel)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redraw the window's frame the way Discord 2026 draws it: server list, guild
header, DM sidebar, Friends/Home with Active Now, chat header, member list, typing line
above the composer, and the user panel with microphone and headset menus.

**Architecture:** New shared widgets first (slider, split button, two-line menu item,
popover container), then one task per area. Logic that decides *what* to show (Active Now
entries, friend filtering, boost goal, guild header rows, typing dots phase) lives in
UI-free functions or stores under `emerald-ui/src/app/state/` and is unit-tested; the
render code consumes it. New guild fields are parsed in `emerald-discord` from data that
already arrives in READY / GUILD_CREATE; this plan adds **no new REST request**.

**Tech Stack:** Rust, gpui 0.2.2, serde, emerald-audio (`devices()`).

**Spec:** `docs/superpowers/specs/2026-10-29-visual-styles-design.md` (goals 3, 4, 8).
Area facts: `docs/superpowers/specs/2026-10-29-visual-styles/gap-shell.md`,
`gap-settings.md` (sections 3–5), and `bundle-plan2.md` (how discord.com does each item).

## Global Constraints

- Every rule in `CLAUDE.md`. No real write to Discord as a test. At most 2 app launches in
  this plan; quit the old instance first. The user checks the screens by hand at the end.
- Every new string in both `emerald-core/locales/en-US.toml` and `pt-BR.toml`
  (`portuguese_is_complete`, `every_used_key_exists` in `emerald-core/src/i18n.rs` enforce it).
  Use Discord's own wording from `bundle-plan2.md` where it gives one.
- Values come from the style: colors from `theme::*`, radii from `theme::radius::*`
  (the guard test forbids fixed radii). New colors go in `theme/colors.rs` with their CSS
  token name in the doc comment.
- Files under ~400 lines; split into a folder when an area grows.
- No Nitro, Shop, Quests, upsells or gift button (spec). The boost goal bar is display-only.
- Nothing that works today is dropped (spec "Calls made"): the Blocked tab stays until
  plan 5 moves it to settings; the member search stays (restyled); the DM filter is
  replaced by the quick switcher, which already finds DMs and opens the first match.
- Idle CPU stays zero: the typing dots animate only while someone is typing, the window is
  active and reduced motion is off, through the existing `animating` /
  `next_animation_frame` pattern (`emerald-ui/src/app/chat/mod.rs`).
- Device listing (`emerald_audio::devices()`) is slow: always on the background executor.
- Commit only when the user asks.

## Review Focus

1. **Slider dragged outside its track** (past either end, or released outside the window)
   clamps to 0..max and stops following the mouse. Test in Task 1.
2. **No audio devices / devices still loading** in the mic/headset menus: the device row
   shows "Default" and the menu still opens. Test in Task 3.
3. **Friends search with accents and case** ("joão" finds "João", "@name" finds the
   username): Test in Task 7.
4. **Active Now with nothing to show** (no friend playing or in voice) shows Discord's
   empty state, and a friend in voice in a guild that went unavailable is skipped. Test in
   Task 8.
5. **Guild without banner / boost bar disabled / no scheduled events**: the plain header is
   drawn, no empty gaps. Test in Task 6.

---

### Task 1: Shared widgets (slider, split button, popover, two-line item)

**Files:**
- Create: `emerald-ui/src/app/widgets/slider.rs`, `emerald-ui/src/app/widgets/popover.rs`
  (turn `widgets.rs` into `widgets/mod.rs` if needed to stay under 400 lines)
- Modify: `emerald-ui/src/theme/colors.rs` (`border_normal()`)
- Modify: `emerald-ui/src/app/place_menu.rs` (`item` keeps working; new `item_two_line`)

**Interfaces:**
- Produces:
  - `theme::border_normal() -> Rgba` — `--border-normal` (light `#dfdfe1`-ish per
    gap-settings, dark from the web client's `.theme-dark` token; find both values in the
    bundle CSS and cite them).
  - `widgets::slider::value_at(x: f32, left: f32, width: f32, max: f32) -> f32` (clamped
    0..=max) and `widgets::slider::slider(id, value: f32, max: f32, on_change:
    impl Fn(f32, &mut Window, &mut App) + 'static) -> impl IntoElement` — 4px track,
    `theme::brand()` fill, `theme::interactive()`-muted rest, 16px white knob with
    `border_normal`; drag with `on_mouse_down` + window-level mouse move/up while pressed.
  - `widgets::split_button(id, icon, on: bool, on_click, on_chevron) -> impl IntoElement`
    — 32px icon part + 16px chevron part, chevron flips while its menu is open.
  - `widgets::popover::panel(id, width: f32) -> Stateful<Div>` — the container style now
    duplicated in `user_menu.rs:153-170` and `place_menu.rs:160-176`
    (`radius::modal`, `border_1`, `border_normal`, `surface_high`, `shadow_lg`, `occlude`);
    switch those two to it.
  - `widgets::popover::anchored_above(position: Point<Pixels>, child) -> impl IntoElement`
    — `deferred(anchored().anchor(Corner::BottomLeft).position(position)
    .snap_to_window_with_margin(8))`.
  - `place_menu::item_two_line(id, title, subtitle, trailing: Option<&'static str>)`.

- [ ] **Step 1: Write the failing tests** (`slider.rs`):

```rust
#[test]
fn value_follows_the_pointer_inside_the_track() {
    assert_eq!(value_at(150., 100., 200., 200.), 50.);
}

#[test]
fn value_clamps_past_either_end() {
    assert_eq!(value_at(50., 100., 200., 200.), 0.);
    assert_eq!(value_at(900., 100., 200., 200.), 200.);
}

#[test]
fn a_zero_width_track_reads_as_zero() {
    assert_eq!(value_at(100., 100., 0., 200.), 0.);
}
```

- [ ] **Step 2: Run** `cargo test -p emerald-ui slider` — expected: FAIL (not defined).
- [ ] **Step 3: Implement the four widgets and move the two menus onto `popover::panel`.**
- [ ] **Step 4: Run** `cargo test -p emerald-ui && cargo clippy -p emerald-ui --all-targets`
  — expected: PASS, no warnings.
- [ ] **Step 5: Commit** `feat: ui: slider, split button and popover widgets`

### Task 2: User panel 2026

**Files:**
- Modify: `emerald-ui/src/app/user_panel.rs:21-101`, `emerald-ui/src/app/voice/panel.rs:117-156`

**Interfaces:**
- Consumes: `widgets::split_button` (Task 1).
- Produces: `Emerald::audio_menu: Option<AudioMenu>` where
  `enum AudioMenuKind { Input, Output }` and
  `struct AudioMenu { kind: AudioMenuKind, position: Point<Pixels> }`; opener
  `Emerald::open_audio_menu(kind, position, cx)`.

Layout (gap-settings 38–39, gap-shell 4.x): background `theme::surface_high()` (the token
doc already says so; today it is `base_low`), height 56, `radius::card()`; order avatar+name,
mic split button, headset split button, gear. The subtitle shows the username while the
avatar block is hovered and the custom status otherwise. Width stays
`72 + sidebar - 16` but reads the sidebar width from one constant shared with
`sidebar/mod.rs` (`SIDEBAR_WIDTH`).

- [ ] **Step 1:** No pure logic here; layout only. Rely on Task 3's tests for the menu.
- [ ] **Step 2: Implement**; the chevrons call `open_audio_menu`.
- [ ] **Step 3: Run** `cargo build && cargo test -p emerald-ui` — expected: PASS.
- [ ] **Step 4: Commit** `feat: ui: 2026 user panel with split mic and headset buttons`

### Task 3: Microphone and headset menus

**Files:**
- Create: `emerald-ui/src/app/voice/audio_menu.rs`
- Modify: `emerald-ui/src/app/render.rs` (render the menu with the other root menus),
  `emerald-ui/src/app/settings/voice.rs` (share the device cache)

**Interfaces:**
- Consumes: `AudioMenu`/`open_audio_menu` (Task 2), `slider`, `popover`, `item_two_line`
  (Task 1), `VoiceSettings { input, output, input_volume, output_volume }`
  (`emerald-ui/src/settings.rs:93-105`), `voice_controls_changed(cx)`
  (`emerald-ui/src/app/voice/mod.rs:84`).
- Produces:
  - `Emerald::audio_devices: Option<emerald_audio::Devices>` — one cache for the menu and
    the Voice settings page (today it lives only on `SettingsView`); filled by
    `list_audio_devices` on the background executor, refreshed each time a menu opens.
  - `fn device_label(devices: Option<&[Device]>, chosen: Option<&str>) -> SharedString` —
    the device's name, else `t!("voice.default_device")`.

Menu (gap-settings 30–37), 220px, anchored above the chevron:
- Input: two-line row "Input Device" / device name with `›` opening a submenu listing
  devices (+ "Default"); separator; "Input Volume" + slider (0..200, value
  `input_volume`); separator; gear + "Voice Settings" → `open_settings(Page::Voice)`.
- Output: same with output device and "Output Volume"; no profile row.
- The 2026 "Input Profile" row (voice isolation / studio) arrives with plan 6, which adds
  the processing; it is not drawn before it works.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test]
fn device_label_names_the_chosen_device() {
    let devices = [Device { id: "a".into(), name: "Realtek(R) Audio".into() }];
    assert_eq!(device_label(Some(&devices), Some("a")).as_ref(), "Realtek(R) Audio");
}

#[test]
fn device_label_falls_back_to_default() {
    assert_eq!(device_label(None, Some("a")).as_ref(), t!("voice.default_device"));
    assert_eq!(device_label(Some(&[]), None).as_ref(), t!("voice.default_device"));
}
```

- [ ] **Step 2: Run** `cargo test -p emerald-ui device_label` — expected: FAIL.
- [ ] **Step 3: Implement** the menu, submenu and cache; slider changes call
  `voice_controls_changed` (it already saves settings and updates a live call).
- [ ] **Step 4: Run** `cargo test -p emerald-ui && cargo clippy -p emerald-ui --all-targets`
  — expected: PASS.
- [ ] **Step 5: Commit** `feat: microphone and headset menus in the user panel`

### Task 4: Typing line above the composer, with animated dots

**Files:**
- Modify: `emerald-ui/src/app/chat/typing.rs`, `emerald-ui/src/app/chat/mod.rs:228-282`

**Interfaces:**
- Produces: `typing::dot_opacity(elapsed_ms: u128, dot: usize) -> f32` — Discord's
  three-dot pulse, period 1400 ms, each dot offset by 200 ms, opacity 0.3..=1.0.

Placement (gap-shell 10.1–10.2, 0.4): the 24px strip under the input goes away; the typing
line overlays the bottom of the message list, left-aligned with the avatar column, 12px
text, bold names, preceded by the three dots (each 4px circle, `theme::text_strong`). The
composer then ends 8px above the window bottom, aligned with the user panel. Dots animate
through `self.animating.set(true)` only when `self.animates()` and someone is typing;
otherwise drawn at full opacity, static.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test]
fn dots_pulse_one_after_another() {
    assert!(dot_opacity(0, 0) > dot_opacity(0, 2));
    assert!(dot_opacity(400, 2) > dot_opacity(400, 0));
}

#[test]
fn dots_stay_within_their_range() {
    for ms in (0..2800).step_by(50) {
        for dot in 0..3 {
            let o = dot_opacity(ms, dot);
            assert!((0.3..=1.0).contains(&o), "{ms} {dot} {o}");
        }
    }
}
```

- [ ] **Step 2: Run** `cargo test -p emerald-ui dots_` — expected: FAIL.
- [ ] **Step 3: Implement** and move the strip.
- [ ] **Step 4: Run** `cargo test -p emerald-ui` — expected: PASS.
- [ ] **Step 5: Commit** `feat: ui: typing line above the composer with animated dots`

### Task 5: Chat header 2026

**Files:**
- Modify: `emerald-ui/src/app/chat/mod.rs:93-198`, `emerald-ui/src/app/panels/mod.rs:113-163`
- Modify: `emerald-ui/src/app/place_menu.rs` (`Page` and a `open_notify_menu` opener
  become reachable)
- Create: `emerald-ui/assets/icons/bell.svg`, `bell_off.svg` (register in `icons.rs`)

**Interfaces:**
- Produces: `Emerald::open_notify_menu(guild: Option<Snowflake>, channel: Snowflake,
  position: Point<Pixels>, cx)` — the existing place menu opened on its Notify page.

Changes (gap-shell 8.1–8.3, bundle-plan2 §5):
- The 1px×24px divider before the topic becomes a `•` in `theme::text_muted()`.
- Tool order: Threads, **Bell**, Pins, member list / profile toggle, then the search box.
  Bell tooltip "Notification Settings"; `bell_off` while the channel is muted
  (`notify.muted(place, now)`); click opens `open_notify_menu`; Shift+click toggles
  mute like the web client. Only in guild channels and threads.
- Search box: 240×32, `radius::control`, 1px `border_normal`, `surface_high` fill,
  magnifier on the **right**, placeholder `t!("search.in_server", name = guild name)`
  ("Search {name}" / "Buscar {name}"); DMs keep "Search".

- [ ] **Step 1: Write the failing test** for the one decision:

```rust
#[test]
fn bell_shows_muted_only_while_the_mute_lasts() {
    // A pure helper in place_menu.rs: bell_icon(muted_until: Option<u64>, now: u64)
    assert_eq!(bell_icon(None, 100), icons::BELL);
    assert_eq!(bell_icon(Some(200), 100), icons::BELL_OFF);
    assert_eq!(bell_icon(Some(50), 100), icons::BELL);
}
```

  (Use whatever "muted until" form `NotifySettings` already exposes; keep the helper
  pure.)
- [ ] **Step 2: Run** `cargo test -p emerald-ui bell_` — expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -p emerald-ui` — expected: PASS.
- [ ] **Step 5: Commit** `feat: ui: 2026 chat header with notification bell`

### Task 6: Server list and guild header

**Files:**
- Modify: `emerald-discord/src/model/guild.rs:107-157` (new fields), `emerald-ui/src/app/state/spaces/guild_info.rs`,
  `emerald-ui/src/app/state/spaces/space.rs:43-77`
- Create: `emerald-ui/src/app/sidebar/guild_header.rs`; `emerald-ui/src/app/state/spaces/header.rs`
- Modify: `emerald-ui/src/app/sidebar/mod.rs:25-54`, `emerald-ui/src/app/sidebar/servers.rs:42-58`
- Create: icons `plus_circle.svg` (outlined), `speaker.svg`

**Interfaces:**
- Produces (emerald-discord, all `#[serde(default)]`, also inside `GuildProperties` since
  READY sends guilds in that shape): `banner: Option<String>`,
  `premium_tier: u8`, `premium_subscription_count: Option<u32>`,
  `premium_progress_bar_enabled: bool`, `guild_scheduled_events: Vec<serde::de::IgnoredAny>`
  counted into `scheduled_events: usize`; `Guild::banner_url(size: u32) -> Option<String>` =
  `cdn/banners/{id}/{hash}.webp?size={size}` (animated hashes `a_` → `.webp?animated=true`,
  as the bundle builds it — confirm in `bundle-plan2.md` §2).
- Produces (UI state): `struct GuildHeader { banner: Option<SharedString>, boost:
  Option<BoostGoal>, events: usize }`, `struct BoostGoal { count: u32, goal: u32 }`,
  `fn boost_goal(count: u32, tier: u8) -> BoostGoal` (goal per bundle-plan2 §2; mark the
  33 total as inferred there), `Space::header() -> Option<&GuildHeader>`.

Server list (gap-shell 1.3–1.4): the "+" becomes a neutral tile (`base_low`) with the
outlined `plus_circle` glyph in `interactive`, hover `text_strong`; a 16px speaker badge at
the icon's top-right when a **friend** is in a voice channel of that guild
(`space.voice_users()` ∩ friends). That condition is a guess; say so in a comment and in the
final report.

Guild header (gap-shell 2.1–2.2): with a banner, a 136px banner image with the name
(16px semibold, white with shadow) over its top, chevron, and an invite button
(`USER_ADD`) on the right calling `open_invite(first invitable channel, "Guild Header", cx)`;
without one, today's 48px header plus the invite button. The boost bar sits under it when
`premium_progress_bar_enabled`: pill, pink fill `count/goal`, label "Boost Goal" and
"{count}/{goal} Boosts ›"; **not clickable** (boosting is a Nitro purchase, excluded). The
Server Guide / Events / Channels & Roles rows need their own pages and land in plan 6.

- [ ] **Step 1: Write the failing tests:**

```rust
// emerald-discord, model/guild.rs
#[test]
fn guild_reads_banner_and_boost_fields() {
    let guild: Guild = serde_json::from_value(json!({
        "id": "1", "name": "g", "banner": "abc", "premium_tier": 2,
        "premium_subscription_count": 14, "premium_progress_bar_enabled": true,
        "guild_scheduled_events": [{}, {}, {}]
    })).unwrap();
    assert_eq!(guild.banner_url(480).unwrap(), "https://cdn.discordapp.com/banners/1/abc.webp?size=480");
    assert_eq!(guild.scheduled_events, 3);
}

#[test]
fn guild_without_the_new_fields_still_parses() {
    let guild: Guild = serde_json::from_value(json!({ "id": "1", "name": "g" })).unwrap();
    assert!(guild.banner_url(480).is_none());
    assert!(!guild.premium_progress_bar_enabled);
}

// emerald-ui, state/spaces/header.rs
#[test]
fn no_banner_no_bar_means_a_plain_header() {
    let header = GuildHeader::from_parts(None, None, false, 0, 0);
    assert!(header.banner.is_none() && header.boost.is_none());
}
```

- [ ] **Step 2: Run** `cargo test -p emerald-discord guild_ && cargo test -p emerald-ui header` — expected: FAIL.
- [ ] **Step 3: Implement** model, state and the two views.
- [ ] **Step 4: Run** `cargo test --workspace` — expected: PASS.
- [ ] **Step 5: Commit** `feat: ui: 2026 server list and guild header with banner and boost goal`

### Task 7: DM sidebar and Friends page

**Files:**
- Modify: `emerald-ui/src/app/sidebar/mod.rs:35-66`, `emerald-ui/src/app/sidebar/dms.rs`,
  `emerald-ui/src/app.rs:270,314-371` (drop `dm_search`), `emerald-ui/src/app/navigation.rs:182-217`
  (`open_first_dm`, `visible_rows`), `emerald-ui/src/app/settings/language.rs:132`
- Modify: `emerald-ui/src/app/friends.rs`, `emerald-ui/src/app/friends/list.rs`
- Create: `emerald-ui/src/app/state/friend_filter.rs`
- Modify: `emerald-discord/src/model/user.rs:70-84` (`Relationship::note: Option<String>`)
- Create: icons `chat_bubble.svg`, `chat_plus.svg`

**Interfaces:**
- Produces: `fn friend_matches(query: &str, name: &str, username: &str) -> bool` —
  case- and accent-insensitive, leading `@` matches the username only.

DM sidebar (gap-shell 3.1, 3.3): the header text field becomes a full-width 32px button
"Find or start a conversation" (`radius::control`, `base_low`, centered 14px) that calls
`open_switcher`. `dm_search`, its `no_results` state, `open_first_dm` and the DMs branch
of `visible_rows` are removed (the switcher finds DMs and Enter opens the first, the same
behavior). "Direct Messages" header 14px medium. Nitro/Shop/Quests rows are **not** added.

Friends (gap-shell 5.x, 6.x):
- Header: friends glyph + "Friends", a `•`, tabs Online, All, Pending (only when there are
  requests), Blocked (kept), then **brand** "Add Friend" (active: brand-tinted bg, brand
  text); new-group-DM `chat_plus` icon on the far right → `open_group_picker(None, ..)`.
  Tabs 8px radius, 16px medium, `px_3 py_1`.
- A 40px search field ("Search", magnifier, `border_normal`) above the list filters with
  `friend_matches`.
- Count label sentence case: `"{label} — {n}"`, 14px medium, muted; update the locale
  values for `friends.online`/`friends.all…` (no longer uppercase).
- Rows: display name only (username appears on row hover), activity line; actions become
  36px round buttons on `base_low`: message (`chat_bubble`) and more (`MORE`) → the user
  menu (`open_user_menu`) at the click; pending rows keep accept/decline/cancel as round
  check/close icons.
- Incoming request with a `note`: an eye button "View Request" reveals the note inline, as
  the web client does (bundle-plan2 §1).
- Add Friend: one bordered 12px-radius box holding the username input and the brand
  "Send Friend Request" button inside on the right; caption below. **The optional message
  field is not drawn yet**: discord.com shows it only to accounts in the experiment
  `2026-03-friend-request-message`, and Emerald does not read experiment assignments; sending
  a note outside it would be traffic the web client never makes. It arrives with the
  experiment reader (added to plan 6). Say this in the final report.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test]
fn friend_search_ignores_case_and_accents() {
    assert!(friend_matches("joao", "João", "jj"));
    assert!(friend_matches("JOÃO", "joão", "jj"));
}

#[test]
fn at_sign_searches_usernames_only() {
    assert!(friend_matches("@jj", "João", "jj"));
    assert!(!friend_matches("@joao", "João", "jj"));
}

#[test]
fn an_empty_query_matches_everyone() {
    assert!(friend_matches("", "a", "b"));
}

// emerald-discord
#[test]
fn relationship_keeps_its_note() {
    let r: Relationship = serde_json::from_value(json!({"id": "1", "type": 3, "note": "oi"})).unwrap();
    assert_eq!(r.note.as_deref(), Some("oi"));
}
```

- [ ] **Step 2: Run** `cargo test --workspace friend_ relationship_keeps` — expected: FAIL.
- [ ] **Step 3: Implement** (accent folding: decompose and drop combining marks with
  `unicode-normalization` if already in the lock, else a small table for Latin letters).
- [ ] **Step 4: Run** `cargo test --workspace` — expected: PASS.
- [ ] **Step 5: Commit** `feat: ui: 2026 DM sidebar and Friends page`

### Task 8: Active Now column

**Files:**
- Create: `emerald-ui/src/app/state/active_now.rs`, `emerald-ui/src/app/friends/active_now.rs`
- Modify: `emerald-ui/src/app/friends.rs` (two columns: list | 360px Active Now)

**Interfaces:**
- Produces:

```rust
pub enum ActiveKind { Voice { guild: Option<Snowflake>, channel: Snowflake }, Playing(SharedString), Listening(SharedString) }
pub struct ActiveEntry { pub user: Snowflake, pub kind: ActiveKind, pub since: Option<u64> }
/// Friends doing something now, as discord.com's Active Now builds it from presences
/// and voice states (custom status excluded); voice first, then newest activity.
pub fn active_now(friends: &[Snowflake], activities: impl Fn(Snowflake) -> Vec<Activity>,
                  voice: impl Fn(Snowflake) -> Option<(Option<Snowflake>, Snowflake)>) -> Vec<ActiveEntry>
```

  The voice lookup is built once per render from `spaces.iter()` + `voice_users()` and
  `dm_calls`. No affinities or `/applications/public` requests (bundle-plan2 §3): those need
  consent and network; game names come from the activity itself.

Layout (gap-shell 5.8): "Active Now" 20px bold; one card per entry (`radius::card`, 1px
`border_normal`, `base_low`): avatar + name + "In a voice channel" / game; voice cards list
the channel and up to 3 avatars + "+N". Empty: "It's quiet for now…" with Discord's
subtitle (find both strings in the bundle). Hidden below 1200px window width.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test]
fn voice_comes_before_games_and_custom_status_is_ignored() { /* friend A playing, B in voice, C custom status only → [B, A] */ }

#[test]
fn nobody_active_means_an_empty_column() { /* no activities, no voice → empty */ }
```

  Write them with concrete `Activity { kind: 0 | 2 | 4, name, .. }` values (kind 4 =
  custom status, 2 = listening, 0 = playing).
- [ ] **Step 2: Run** `cargo test -p emerald-ui active_now` — expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -p emerald-ui` — expected: PASS.
- [ ] **Step 5: Commit** `feat: ui: Active Now column on the Friends page`

### Task 9: Member list 2026

**Files:**
- Modify: `emerald-ui/src/app/members.rs:34,132-162`, `emerald-ui/src/app/members/row.rs:28-43,177-188`

Changes (gap-shell 9.1, 9.4–9.5): group labels keep the role's own case (drop the
`to_uppercase` at `row.rs:187`; the built-in "Online"/"Offline" labels in sentence case),
13px medium; width 256; background `base_lower` (same as the chat) with the left border;
the search input moves into a 32px row styled like the header search (border, icon right)
so it stays available. The content-inventory **Activity** section (bundle-plan2 §4) is a
new request with its own refresh event; it lands in plan 3 with the other data work.

- [ ] **Step 1: Write the failing test:**

```rust
#[test]
fn role_groups_keep_their_case() {
    // group_label for a role named "Classe S" returns "Classe S", not "CLASSE S".
}
```

  (Extract the label decision into a pure `fn group_title(role_name: Option<&str>, id: &str) -> String`.)
- [ ] **Step 2: Run** `cargo test -p emerald-ui role_groups` — expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -p emerald-ui` — expected: PASS.
- [ ] **Step 5: Commit** `feat: ui: 2026 member list`

### Task 10: Look at it, README, final checks

- [ ] `cargo build`, `cargo test --workspace`, `cargo clippy --workspace --all-targets` — clean
  (the old `emerald-desktop` test warning aside).
- [ ] One launch (quit the old instance first): capture the window in the background with
  `capture.ps1` (PrintWindow) on Friends and on one server channel the user already has
  open as last channel; compare with `themes/2026/home.png` and
  `uma_servidor_….png`. No clicks that write anything.
- [ ] `README.md`: "What works" — user panel menus, Active Now, notification bell, guild
  banner/boost goal; "Not there yet" — Server Guide/Events/Channels & Roles rows,
  member-list Activity, friend-request note, input profile.
- [ ] `grep -rn DEBUG emerald-*/src` — nothing left.
- [ ] Report to the user (Portuguese): what was seen, what needs their hands (menus,
  slider drag, bell Shift+click), and the guesses (speaker badge condition, boost goal 33).
