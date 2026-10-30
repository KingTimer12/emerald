# Visual styles, plan 1 of 6: style foundation, font and custom title bar

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every color, radius and the UI font come from an active visual style (only
2026 exists yet), bundle a free gg-sans look-alike font, fix light-theme button text, and
replace the operating system's title bar with Emerald's own 2026 title bar (back/forward,
centered page title, Inbox, Help, window controls).

**Architecture:** `emerald-ui/src/theme.rs` becomes a `theme/` folder: `mod.rs` keeps the
global switches (light, style) and `pick`; colors, radii and fonts move to their own
modules. Screens stop calling gpui's fixed `rounded_*()` helpers and call semantic radius
functions instead, so a later style changes values in one place. Navigation history is a
UI-free store in `app/state/`, and the title bar is a new `app/title_bar.rs` rendered at
the top of the root view, using gpui's `WindowControlArea` for drag and the window buttons.

**Tech Stack:** Rust, gpui 0.2.2 (`TitlebarOptions::appears_transparent`,
`WindowControlArea`, `TextSystem::add_fonts`), serde, ttf-parser (dev-dependency, already
in `Cargo.lock`).

**Spec:** `docs/superpowers/specs/2026-10-29-visual-styles-design.md` (goals 1 and 2,
the light-button fix from goal 8, and the font decision).

### The six plans of project 1

1. **This plan:** foundation, font, title bar, history.
2. **Shell:** server list (neutral "+" glyph; the Discover button comes with plan 6),
   guild header (banner, invite, boost bar, guide rows), DM sidebar, friends/home
   (Active Now, search, round actions, Add Friend box with optional message), chat header,
   member list (activity section, search moved), typing above composer, user panel split
   mic/headset buttons with device menus and a slider widget.
3. **Messages and people:** timestamps, hover bar (forward, Shift actions, Edit on own),
   user popout, full profile (two columns, widgets), DM profile panel, role icons, server
   tags, display-name styles, profile theme colors.
4. **Media and expressions:** image viewer, composer bar, pickers (rail, locked items,
   animated GIFs, GIF favorites), Twemoji, embeds, alt text / remove attachment.
5. **Settings and account:** settings modal and sidebar, Account rows (phone, password,
   two-factor), profile editor in three columns with Nitro items locked.
6. **Discover, resizable sidebar, microphone input profiles.**

Each later plan is written when the previous one lands, from the gap reports in
`docs/superpowers/specs/2026-10-29-visual-styles/`.

## Global Constraints

- Every rule in `CLAUDE.md` applies. Do not launch the app in loops; at most 3 launches in
  this plan, each after `taskkill /IM emerald.exe /F` (Windows) so two sessions never run.
- Every new interface string goes in both `emerald-core/locales/en-US.toml` and
  `emerald-core/locales/pt-BR.toml`, written by hand.
- Code, comments, commits in English; doc comments on items; no abbreviations in names.
- Files under ~400 lines, one concern each.
- `cargo build`, `cargo test`, `cargo clippy` with no new warnings before each commit.
- Commit only when the user asks; conventional prefixes; never a `Co-Authored-By` line.
- Default style is 2026. Dark / light / system stays its own setting and applies on top.
- The bundled font must have an open licence (OFL or similar); its licence file ships
  next to it. Discord's gg sans and Whitney are never bundled.
- Windows is the primary check target; macOS must still build and work.
- Idle CPU stays at zero: nothing in the title bar redraws on a timer.

## Review Focus

1. **A `settings.json` with an unknown `style` value** (written by a newer build, e.g.
   `"2018"` before 2018 exists in this build) must load every other setting and fall back
   to 2026; today an unknown enum value makes `Settings::load` return all defaults.
   Test in Task 1.
2. **Maximized window and the title bar:** maximize/restore button icon follows the real
   state, double-click on the drag area maximizes/restores, and the controls stay
   clickable at every zoom level (90%–150%). Covered by the manual check in Task 6.
3. **History with deleted or inaccessible places:** going back to a channel that was
   deleted or that the user lost access to skips it instead of opening an empty chat.
   Test in Task 5.
4. **Light theme on colored buttons:** brand, danger and positive buttons draw white text
   in both palettes. Test in Task 3.
5. **Font missing or failing to load** (corrupt asset): the app still starts with the
   system font. Test in Task 4.

---

### Task 1: Style switch and its setting

**Files:**
- Move: `emerald-ui/src/theme.rs` → `emerald-ui/src/theme/mod.rs` (switches, `pick`) and
  `emerald-ui/src/theme/colors.rs` (every color function, re-exported with `pub use
  colors::*` so `theme::brand()` keeps working)
- Modify: `emerald-ui/src/settings.rs` (new `style` field)
- Modify: `emerald-ui/src/app/window.rs:66-72` (`apply_appearance` also sets the style)
- Test: in `emerald-ui/src/theme/mod.rs` and `emerald-ui/src/settings.rs`

**Interfaces:**
- Produces:
  - `pub enum Style { Y2026 }` in `emerald-ui/src/settings.rs`, `#[derive(Clone, Copy,
    Debug, Default, PartialEq, Eq, Serialize)]`, `#[serde(rename = "2026")]` on `Y2026`,
    default `Y2026`, with a hand-written `Deserialize` that maps any unknown string to
    `Y2026`. `pub const ALL: [Style; 1]`.
  - `Settings::style: Style`.
  - `theme::set_style(style: Style)`, `theme::style() -> Style` (an `AtomicU8`, like
    `LIGHT`).
  - `theme::pick` unchanged in meaning for 2026 (dark, light); later styles add their
    own tables.

- [ ] **Step 1: Write the failing tests**

```rust
// settings.rs
#[test]
fn unknown_style_falls_back_without_losing_other_settings() {
    let settings: Settings = serde_json::from_str(r#"{"style":"1999","font_size":18.0}"#).unwrap();
    assert_eq!(settings.style, Style::Y2026);
    assert_eq!(settings.font_size, 18.);
}

#[test]
fn style_round_trips_as_its_year() {
    let json = serde_json::to_string(&Settings::default()).unwrap();
    assert!(json.contains(r#""style":"2026""#));
}

// theme/mod.rs
#[test]
fn style_switch_is_read_back() {
    set_style(Style::Y2026);
    assert_eq!(style(), Style::Y2026);
}
```

- [ ] **Step 2: Run** `cargo test -p emerald-ui style` — expected: FAIL, `Style` not found.
- [ ] **Step 3: Split `theme.rs` into the folder, add `Style`, the lenient `Deserialize`,
  `set_style`/`style`, and set it in `apply_appearance`.** Keep the existing
  `light_palette_only_while_switched` test passing unchanged.
- [ ] **Step 4: Run** `cargo test -p emerald-ui` — expected: PASS.
- [ ] **Step 5: Commit** `refactor: theme: folder per concern, style switch in settings`

### Task 2: Semantic radii instead of gpui's fixed ones

**Files:**
- Create: `emerald-ui/src/theme/radius.rs`
- Modify: every file under `emerald-ui/src` that calls `rounded_sm/md/lg/xl()`,
  `rounded(…)`, `rounded_t*(…)`, `rounded_b*(…)` (about 85 call sites; `rounded_full()`
  stays, it means "circle" in every style)
- Test: `emerald-ui/src/theme/radius.rs`

**Interfaces:**
- Produces, all `pub fn …() -> Pixels`, zoomed through `crate::zoom::zoomed`, values for
  2026 measured in the gap reports:

| function | 2026 px | used for |
|---|---|---|
| `radius::tag()` | 4 | app/bot tags, server tags, small chips, badges |
| `radius::control()` | 8 | buttons, inputs, search fields, list rows, tabs, hover toolbar |
| `radius::card()` | 8 | embeds, attachments, popover cards, user panel, panel corner |
| `radius::server_icon()` | 12 | server icons and the home button |
| `radius::folder()` | 16 | open server folder |
| `radius::modal()` | 12 | modals, settings window, popouts, pickers |

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn the_2026_radii_match_the_screenshots() {
    crate::zoom::set(100);
    crate::theme::set_style(Style::Y2026);
    assert_eq!(control(), px(8.));
    assert_eq!(server_icon(), px(12.));
    assert_eq!(modal(), px(12.));
    assert_eq!(tag(), px(4.));
}

/// Guards the migration: screens use the style's radii, not gpui's fixed ones.
#[test]
fn screens_do_not_use_fixed_radii() {
    let fixed = ["rounded_sm()", "rounded_md()", "rounded_lg()", "rounded_xl()", ".rounded(px("];
    let offenders: Vec<String> = walk(concat!(env!("CARGO_MANIFEST_DIR"), "/src"))
        .filter(|(path, _)| !path.ends_with("radius.rs"))
        .filter(|(_, text)| fixed.iter().any(|f| text.contains(f)))
        .map(|(path, _)| path)
        .collect();
    assert!(offenders.is_empty(), "fixed radii in {offenders:?}");
}
```

  `walk` is a small test-only helper returning `(path, contents)` for every `.rs` file.

- [ ] **Step 2: Run** `cargo test -p emerald-ui radius` — expected: FAIL.
- [ ] **Step 3: Write `radius.rs`, then replace every fixed radius** with the semantic one
  matching its use (table above). Where today's value differs from the 2026 table, the
  table wins (for example friends tabs move from 6 to 8). Corner-only calls become
  `.rounded_tl(theme::radius::card())` and so on.
- [ ] **Step 4: Run** `cargo test -p emerald-ui && cargo clippy -p emerald-ui` — expected:
  PASS, no new warnings.
- [ ] **Step 5: Commit** `refactor: ui: radii come from the active style`

### Task 3: White text on colored buttons in both palettes

**Files:**
- Modify: `emerald-ui/src/app/widgets.rs:146-176`
- Modify: `emerald-ui/src/theme/colors.rs` (new `on_color()`)

**Interfaces:**
- Produces: `theme::on_color() -> u32` = `0xffffff` (`--white`, text on brand/danger/
  positive fills); `widgets::button_text(kind: ButtonKind) -> u32`, used by `button`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn colored_buttons_use_white_text_in_both_palettes() {
    for light in [false, true] {
        theme::set_light(light);
        for kind in [ButtonKind::Brand, ButtonKind::Positive, ButtonKind::Danger] {
            assert_eq!(button_text(kind), 0xffffff);
        }
        assert_eq!(button_text(ButtonKind::Secondary), theme::text_strong());
    }
    theme::set_light(false);
}
```

- [ ] **Step 2: Run** `cargo test -p emerald-ui colored_buttons` — expected: FAIL.
- [ ] **Step 3: Add `on_color`, `button_text`, use it in `button`.** Also give
  `ButtonKind::Secondary` the 1px `theme::border()` outline seen in the settings
  screenshots (gap-settings.md, "secondary buttons").
- [ ] **Step 4: Run** `cargo test -p emerald-ui` — expected: PASS.
- [ ] **Step 5: Commit** `fix: ui: white text on colored buttons in the light theme`

### Task 4: Bundled open-licence font

**Files:**
- Create: `emerald-ui/assets/fonts/<Family>-Variable.ttf` (or 4 static weights 400, 500,
  600, 700) and `emerald-ui/assets/fonts/OFL.txt`
- Create: `emerald-ui/src/theme/fonts.rs`
- Modify: `emerald-ui/src/lib.rs` (a `pub fn load_fonts(cx: &mut App)`), call it from
  `emerald-bin/src/main.rs` inside `app.run` before `open_window`
- Modify: `emerald-ui/src/app/render.rs:61-66` (root `.font_family(theme::fonts::sans())`)
- Modify: `emerald-ui/Cargo.toml` (`ttf-parser = "0.25"` under `[dev-dependencies]`)
- Modify: `README.md` (credit the font and its licence)

**Interfaces:**
- Produces: `theme::fonts::sans() -> &'static str` (family name for the active style),
  `theme::fonts::BUNDLED: &[&[u8]]`, `emerald_ui::load_fonts(cx: &mut App)`; `MONO`
  moves into `fonts.rs` unchanged.

- [ ] **Step 1: Choose the font.** Candidates, all OFL on Google Fonts' GitHub: Figtree,
  Nunito Sans, Noto Sans. Render "Mensagens diretas Amigos Wumpus está digitando… 21:22"
  with each at 16px (a throwaway Python/PIL script in the scratchpad, run with `-I`) and
  compare with the same words cropped from `themes/2026/home.png` and
  `themes/2026/veja_onde_fica_digitando_atualmente.png`. Pick the closest (x-height, round
  "a", narrow width) and note the choice and why in `fonts.rs`' module doc. Download only
  from `github.com/google/fonts` or the family's official repository.
- [ ] **Step 2: Write the failing tests**

```rust
#[test]
fn bundled_font_family_matches_the_name_the_ui_asks_for() {
    for bytes in BUNDLED {
        let face = ttf_parser::Face::parse(bytes, 0).unwrap();
        let family = face.names().into_iter()
            .find(|name| name.name_id == ttf_parser::name_id::FAMILY && name.is_unicode())
            .and_then(|name| name.to_string()).unwrap();
        assert_eq!(family, sans());
    }
}

#[test]
fn bundled_font_covers_portuguese() {
    let face = ttf_parser::Face::parse(BUNDLED[0], 0).unwrap();
    for ch in "ãçéêíóõúÁÉÍÓÚ…•—".chars() {
        assert!(face.glyph_index(ch).is_some(), "missing {ch}");
    }
}
```

- [ ] **Step 3: Run** `cargo test -p emerald-ui bundled_font` — expected: FAIL.
- [ ] **Step 4: Implement.** `BUNDLED` uses `include_bytes!`. `load_fonts` calls
  `cx.text_system().add_fonts(...)` and on error logs `warn!` (no panic) so the system
  font remains; gpui falls back by itself when the family is unknown. Code blocks keep
  `MONO`.
- [ ] **Step 5: Run** `cargo test -p emerald-ui && cargo build` — expected: PASS.
- [ ] **Step 6: Commit** `feat: ui: bundle an open-licence look-alike of Discord's font`

### Task 5: Navigation history (back / forward)

**Files:**
- Create: `emerald-ui/src/app/state/nav_history.rs` (UI-free; add `mod` + `pub use` in
  `app/state/mod.rs`)
- Modify: `emerald-ui/src/app/navigation.rs` (`select_channel` and the friends opening in
  `select_space` record a place; new `go_back` / `go_forward`)
- Modify: `emerald-ui/src/app.rs` (field `history_nav: NavHistory` on `Emerald`)
- Modify: `emerald-ui/src/shortcuts.rs` (actions `GoBack`, `GoForward`; default bindings
  `alt-left` / `alt-right` on Windows, `cmd-[` / `cmd-]` on macOS, as Discord's desktop
  client; configurable like the others)
- Locales: shortcut names `shortcuts.go_back`, `shortcuts.go_forward` in both files

**Interfaces:**
- Produces:

```rust
/// A place the user can go back to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place { Friends, Channel(Snowflake) }

pub struct NavHistory { /* back: Vec<Place>, forward: Vec<Place>, current: Option<Place> */ }
impl NavHistory {
    pub const LIMIT: usize = 50;
    pub fn visit(&mut self, place: Place);            // same as current: no-op; clears forward
    pub fn back(&mut self, exists: impl Fn(Place) -> bool) -> Option<Place>;
    pub fn forward(&mut self, exists: impl Fn(Place) -> bool) -> Option<Place>;
    pub fn can_go_back(&self) -> bool;
    pub fn can_go_forward(&self) -> bool;
}
```

  `exists` lets the caller skip channels that were deleted or can no longer be read.
  `go_back`/`go_forward` on `Emerald` navigate without calling `visit` again.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn back_and_forward_walk_the_visits() {
    let mut h = NavHistory::default();
    h.visit(Place::Friends);
    h.visit(Place::Channel(Snowflake(1)));
    h.visit(Place::Channel(Snowflake(2)));
    assert_eq!(h.back(|_| true), Some(Place::Channel(Snowflake(1))));
    assert_eq!(h.back(|_| true), Some(Place::Friends));
    assert_eq!(h.back(|_| true), None);
    assert_eq!(h.forward(|_| true), Some(Place::Channel(Snowflake(1))));
}

#[test]
fn a_new_visit_drops_the_forward_stack() {
    let mut h = NavHistory::default();
    h.visit(Place::Friends);
    h.visit(Place::Channel(Snowflake(1)));
    h.back(|_| true);
    h.visit(Place::Channel(Snowflake(3)));
    assert!(!h.can_go_forward());
}

#[test]
fn revisiting_the_current_place_is_not_a_step() {
    let mut h = NavHistory::default();
    h.visit(Place::Friends);
    h.visit(Place::Friends);
    assert!(!h.can_go_back());
}

#[test]
fn back_skips_places_that_no_longer_exist() {
    let mut h = NavHistory::default();
    h.visit(Place::Friends);
    h.visit(Place::Channel(Snowflake(9)));
    h.visit(Place::Channel(Snowflake(2)));
    assert_eq!(h.back(|p| p != Place::Channel(Snowflake(9))), Some(Place::Friends));
}

#[test]
fn history_is_bounded() {
    let mut h = NavHistory::default();
    for id in 0..200 { h.visit(Place::Channel(Snowflake(id))); }
    let mut steps = 0;
    while h.back(|_| true).is_some() { steps += 1; }
    assert_eq!(steps, NavHistory::LIMIT);
}
```

- [ ] **Step 2: Run** `cargo test -p emerald-ui nav_history` — expected: FAIL.
- [ ] **Step 3: Implement `NavHistory`, then wire `visit` into navigation and the two
  actions.** `exists` for a channel: the channel is in the cache and readable
  (`permissions` check already used by `select_channel`).
- [ ] **Step 4: Run** `cargo test -p emerald-ui` — expected: PASS.
- [ ] **Step 5: Commit** `feat: back and forward through visited channels`

### Task 6: Custom title bar (2026)

**Files:**
- Create: `emerald-ui/src/app/title_bar.rs` (render) and
  `emerald-ui/src/app/title_bar/controls.rs` (Windows minimize / maximize / close)
- Create: icons `assets/icons/arrow_left.svg`, `arrow_right.svg`, `help.svg`,
  `window_minimize.svg`, `window_maximize.svg`, `window_restore.svg`, `window_close.svg`
  (registered in `emerald-ui/src/icons.rs` `ALL`)
- Modify: `emerald-bin/src/main.rs:120-123` (`TitlebarOptions { title: Some("Emerald"),
  appears_transparent: true, traffic_light_position: Some(point(px(12.), px(10.))) }`)
- Modify: `emerald-ui/src/app/render.rs` (root becomes a column: title bar, then today's
  row)
- Modify: `emerald-ui/src/app/panels/mod.rs:163-169` (Inbox button moves out of the chat
  header into the title bar)
- Locales: `title_bar.back`, `title_bar.forward`, `title_bar.help`, `title_bar.minimize`,
  `title_bar.maximize`, `title_bar.restore`, `title_bar.close` in both files

**Interfaces:**
- Consumes: `NavHistory::can_go_back/can_go_forward`, `Emerald::go_back/go_forward`
  (Task 5); `theme::radius::control()` (Task 2).
- Produces: `Emerald::render_title_bar(&self, window: &Window, cx) -> AnyElement`;
  `Emerald::page_title(&self) -> (TitleIcon, SharedString)` where
  `enum TitleIcon { Friends, Guild(Snowflake), User(Snowflake), Group(Snowflake) }`.

Layout (2026, from gap-shell.md 0.1, light screenshot measures): height 32px (zoomed),
background `theme::base_lowest()`, no bottom border. Left: on macOS a 72px gap for the
traffic lights, then back and forward 24px icon buttons (`theme::interactive()`, disabled
at 40% opacity when the history says so). Center: 16px icon (friends glyph, guild icon,
or avatar) + 14px medium title, centered on the window, truncated. Right: Inbox and Help
24px icon buttons, then on Windows three 46×32 controls; close hovers `#e81123` with white
glyph. The whole empty bar is `WindowControlArea::Drag`; each Windows control area is
`Min` / `Max` / `Close` so Windows snap layouts work. Help opens
`https://support.discord.com` through the existing open-link path (host from `urls.rs`).

- [ ] **Step 1: Write the failing test** for the title text, which is the only logic:

```rust
#[test]
fn page_title_names_the_place_on_screen() {
    // Uses the existing fake-state helpers of app tests.
    let mut app = test_emerald();
    app.friends_open = true;
    assert_eq!(app.page_title().1.as_ref(), t!("friends.title").as_ref());
    app.open_fake_guild_channel("illusia", "bate-papo");
    assert_eq!(app.page_title().1.as_ref(), "illusia");
}
```

  If `Emerald` cannot be built in a unit test, move `page_title` into a function over
  `&Spaces` and the selected channel and test that instead.

- [ ] **Step 2: Run** `cargo test -p emerald-ui page_title` — expected: FAIL.
- [ ] **Step 3: Implement the title bar, controls and icons; switch the window options.**
- [ ] **Step 4: Run** `cargo build && cargo test && cargo clippy` — expected: PASS.
- [ ] **Step 5: Look at it once in the real app (launch 1 of ≤3).** Quit any old
  instance, start `target/debug/emerald.exe`, capture the window in the background without
  focusing it (PowerShell + `PrintWindow` with `PW_RENDERFULLCONTENT` on the app's
  `MainWindowHandle`, saved to the scratchpad), and compare against
  `themes/2026/home.png`. Then check by hand on screen, without sending anything:
  drag, double-click to maximize/restore, the restore icon when maximized, minimize,
  close, snap layout on hovering maximize, back/forward between two channels, at zoom 100%
  and 150%. Quit the app.
- [ ] **Step 6: Update `README.md`** ("What works": custom title bar, back/forward,
  bundled font) **and commit** `feat: ui: Discord 2026 title bar drawn by the app`

### Task 7: Final checks for this plan

- [ ] `cargo build`, `cargo test`, `cargo clippy` at the workspace root — all clean.
- [ ] `grep -rn DEBUG emerald-*/src` — no leftover debug hooks.
- [ ] Tell the user (in Portuguese) what was checked in the app and what was not, and ask
  before writing plan 2.
