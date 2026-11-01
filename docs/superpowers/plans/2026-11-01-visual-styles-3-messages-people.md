# Visual styles, plan 3 of 6: messages and people

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make messages and people look and work as in Discord 2026: message header times,
the 2026 hover bar with its Shift-expanded form, forwarding, bookmarks and reminders (with
the Inbox's Favorites and Reminders tabs), server tags, role icons and display-name fonts,
profile theme colors, the user popout, the two-column full profile, the DM profile panel,
and the member list's Activity section.

**Architecture:** Data first: new fields are parsed in `emerald-discord` (models with
serde tests) and new requests go in `packets/` + `rest/` + `commands/` behind `Command` /
`NetEvent`. Decisions (what a reminder option resolves to, a theme's colors, which
buttons the hover bar shows, which activity cards a member list keeps) are UI-free
functions or stores under `emerald-ui/src/app/state/`, unit-tested. Render code consumes
them. Popovers use `widgets::popover::{Anchor, Tracked, at}` from plan 2.

**Tech Stack:** Rust, gpui 0.2.2, serde, existing `Call` REST layer, `Alerts` for desktop
notifications.

**Spec:** `docs/superpowers/specs/2026-10-29-visual-styles-design.md` (goals 3, 4, 5, 7, 8).
Area facts: `docs/superpowers/specs/2026-10-29-visual-styles/gap-messages.md`;
how discord.com does each item: `bundle-plan3-messages.md` (part A) and
`bundle-plan3-people.md` (part B), and `bundle-plan2.md` section 4 (Activity section).
Section numbers below like "A§1" or "B§3" point into those two files.

## Global Constraints

- Every rule in `CLAUDE.md`. No real write to Discord as a test: bookmarking, reminders,
  forwarding, marking unread and the popout message box are verified by unit tests and then
  handed to the user. Visual checks use the release build (`cargo r -r` opens the user's
  account) with a temporary `EMERALD_X_TEST` DEBUG-HOOK, read-only, at most 4 launches,
  one instance at a time, hook removed at the end (`grep -rn DEBUG emerald-*/src` empty).
- Every new string in both locale files, with Discord's own wording from parts A/B.
- Colors from `theme::*`, radii from `theme::radius::*`, files under ~400 lines.
- No Nitro, Shop, Quests, Orbs or upsells. Other people's Nitro cosmetics (theme colors,
  name fonts, tags) are displayed; editing them is plan 5.
- Imitate the web client's requests exactly (routes, bodies, query strings, `location` /
  `source` / `X-Context-Properties`). New request bodies in `emerald-discord/src/packets/`.
  A value not found in the bundle is marked as a guess in a comment and told to the user.
- Bounded caches and zero idle CPU: games, role icons, tag badges and fonts are fetched
  once and kept in bounded maps; the reminder clock wakes only at the next due time.
- Forwarding never opens a new DM (a write): the destination list holds existing DMs,
  group DMs and channels only.
- Commit only when the user asks.

## Review Focus

1. **A reminder due while Emerald was closed** fires once on the next start (overdue),
   never repeatedly, and a reminder removed on another device disappears without firing.
   Test in Task 5.
2. **Forward to 5 destinations where one fails** (no permission, slowmode): the others
   still go, the optional text is sent only after its forward succeeded, and the failure is
   reported once. Test in Task 3.
3. **Profile theme with a near-white or near-black primary color**: text keeps readable
   contrast (light card picks dark text and the reverse). Test in Task 8.
4. **Unknown ids**: an unknown display-name font id, effect id, widget type or content
   type is ignored (plain name / card skipped), never a parse failure of the whole user or
   profile. Test in Task 6.
5. **Shift held while the pointer leaves the message** or the window loses focus: the bar
   returns to its normal form; no stuck expanded bar. Test in Task 4.

---

### Task 1: Message header times

**Files:**
- Modify: `emerald-ui/src/time.rs` (`header`), `emerald-ui/src/app/chat/list.rs:265-335`
- Modify: locales (`time.today` no longer used in headers; keep the key if other callers
  use it, else remove it from both files)

**Interfaces:**
- Produces: `time::header(secs: i64) -> String` returning only the clock for today
  (`"21:22"` / `"9:22 PM"` per `TimeFormat`), `t!("time.yesterday", time)` for yesterday,
  and the numeric date + clock otherwise (A§4). `time::clock_of(secs: i64) -> String`
  (the `LT` form) for the gutter.

- [ ] **Step 1: Failing tests** in `time.rs`: `header_today_is_only_the_clock`
  (now − 60 s → equals `clock_of(now − 60)`), `header_yesterday_keeps_the_word`
  (now − 86 400 → contains `clock_of(...)` and is not equal to it).
- [ ] **Step 2: Run** `cargo test -p emerald-ui time::` — FAIL.
- [ ] **Step 3: Implement** the two functions.
- [ ] **Step 4: Rows.** In `list.rs`: name 16px medium, time 12px muted, 8px gap (`gap_2`)
  (M7). Follow-up rows draw `clock_of(msg.secs)` in the 72px gutter, 11px muted,
  right-aligned, visible only while the row is hovered (`group_hover`) (M6). Store the
  gutter string on `Msg` next to `time` (computed once in `Msg::from`, like `time`).
- [ ] **Step 5: Run** `cargo test -p emerald-ui` — PASS.

### Task 2: Saved messages (bookmarks and reminders) — data and requests

**Files:**
- Create: `emerald-discord/src/model/saved.rs`, `emerald-discord/src/packets/saved.rs`,
  `emerald-discord/src/rest/saved.rs`, `emerald-discord/src/commands/saved.rs`,
  `emerald-ui/src/app/state/saved.rs`
- Modify: `protocol/command.rs`, `protocol/event.rs`, `commands/mod.rs`,
  `gateway/dispatch` (two new events), `emerald-ui/src/app/events.rs`, `app.rs`

**Interfaces:**
- Produces (emerald-discord), per A§1:
  - `model::SavedMessage { message: Option<Message>, save_data: SaveData }`,
    `SaveData { channel_id, message_id, saved_at: String, guild_id: Option<Snowflake>
    (0 → None), author_id: Option<Snowflake> (0 → None), due_at: Option<String>,
    author_summary, channel_summary, message_summary: Option<String> }`.
  - `Command::SavedMessages` → `GET /users/@me/saved-messages?bookmark_ids=true` →
    `NetEvent::SavedMessages(Result<SavedList, String>)`, `SavedList { reminders:
    Vec<SavedMessage>, bookmark_ids: Vec<Snowflake> }`. Sent once after READY.
  - `Command::Bookmarks { before: Option<String> }` → `GET .../bookmarks?before=&limit=25`
    → `NetEvent::Bookmarks { before, page: Result<BookmarkPage, String> }`,
    `BookmarkPage { results: Vec<SavedMessage>, has_more: bool }`.
  - `Command::SaveMessage { channel, message, due_at: Option<String>, source: SaveSource }`
    → `PUT /users/@me/saved-messages/{channel}/{message}` body `{due_at?, source}`
    (`due_at` left out for a bookmark). `SaveSource` serializes to `context_menu`,
    `message_toolbar`, `for_later_list`, `notifications_inbox`.
  - `Command::UnsaveMessage { channel, message }` → `DELETE` same route.
  - Gateway `SAVED_MESSAGE_CREATE` → `NetEvent::SavedMessageCreate(SavedMessage)`,
    `SAVED_MESSAGE_DELETE` → `NetEvent::SavedMessageDelete { channel_id, message_id }`.
  - Error code 30074 maps to a worded "limit reached" error (50 reminders / 500 bookmarks).
- Produces (emerald-ui) `state::saved::Saved`:
  `load(SavedList)`, `upsert(SavedMessage)`, `remove(channel, message)`,
  `is_bookmarked(message) -> bool`, `reminder(message) -> Option<&SavedMessage>`,
  `reminders() -> &[SavedMessage]` sorted by `due_at` ascending,
  `next_due(now_ms) -> Option<u64>`, `take_due(now_ms) -> Vec<SavedMessage>` (each
  reminder returned once; remembers fired ids).

- [ ] **Step 1: Failing tests:** serde of the A§1 sample JSON (ids `0` → `None`),
  packet body with and without `due_at`, and `Saved` tests:
  `bookmark_then_remove`, `reminders_sorted_by_due`, `due_fires_once`,
  `deleted_before_due_never_fires`.
- [ ] **Step 2: Run** `cargo test -p emerald-discord saved` and
  `cargo test -p emerald-ui saved` — FAIL.
- [ ] **Step 3: Implement** models, packet, rest (writes through `Call` with the user's
  `major`), commands, dispatch and the store; post `Command::SavedMessages` where READY is
  handled.
- [ ] **Step 4: Run** both test commands — PASS.

### Task 3: Forward message

**Files:**
- Create: `emerald-ui/src/app/forward.rs` (modal), `emerald-ui/src/app/state/forward.rs`
- Modify: `emerald-discord/src/packets/message.rs` (`Outgoing.forward`),
  `commands/message.rs`, `protocol/command.rs`, `app.rs` (`Overlay::Forward`)

**Interfaces:**
- Produces:
  - `Command::Forward { from: ReferenceTo { guild, channel, message }, to: Vec<Snowflake>,
    text: Option<String>, silent: bool }`. Per destination: `POST /channels/{to}/messages`
    with `content: ""`, a nonce and `message_reference {guild_id, channel_id, message_id,
    type: 1}`; header `X-Context-Properties` location `"forwarding"` (A§2). When `text` is
    set, a second normal send follows **only after** the first succeeded (skipped when the
    channel has slowmode and the user lacks bypass). Answer:
    `NetEvent::Forwarded { failed: Vec<Snowflake> }`.
  - `state::forward::destinations(query: &str, recent: &[Snowflake], all: &[Dest]) ->
    Vec<Dest>`: no query → recent first, then the rest, capped at 15; with a query →
    name matches (reuse `friend_filter`'s folding). `Dest { channel, label, kind }` from
    existing DMs, group DMs and sendable guild channels only.
  - `Emerald::open_forward(channel, message, window, cx)`.
- Modal (A§2): search box, list with checkboxes (max 5, extra picks disabled), message
  preview, "Add an optional message..." field, button "Send" / "Send (N)", toast
  "Message forwarded!" on success.

- [ ] **Step 1: Failing tests:** packet body has `type: 1` and empty content;
  `destinations` (recent first, cap 15, query folding); a `Forward` plan helper
  `forward_steps(to, text) -> Vec<Step>` puts each text send right after its forward.
- [ ] **Step 2: Run** — FAIL. **Step 3: Implement.** **Step 4: Run** — PASS.
- [ ] **Step 5:** Open it from the message context menu ("Forward", icon `FORWARD`).

### Task 4: Hover bar and message menu 2026

**Files:**
- Create: `emerald-ui/src/app/chat/actions/bar.rs` (move the toolbar out of `menu.rs`)
- Create: `emerald-ui/src/app/state/hover_bar.rs`
- Modify: `chat/actions/menu.rs`, `emerald-discord` (`Command::MarkUnread`),
  icons (`link.svg`, `bookmark.svg`, `bookmark_filled.svg`, `clock.svg`, `mark_unread.svg`)

**Interfaces:**
- Produces:
  - `state::hover_bar::buttons(own: bool, expanded: bool, rights: Rights, developer: bool)
    -> Vec<BarButton>` in A§3 order. Normal: 3 quick reactions, divider, AddReaction,
    Reply (others) / Edit (own), Forward, Thread, More. Expanded (Shift held): CopyId
    (only with developer mode — keep it always on in Emerald, it is there today), CopyLink,
    MarkUnread, Bookmark, Pin, CreateThread, Reply (own), then Delete in More's place when
    allowed.
  - `Command::MarkUnread { channel, before: Snowflake, mentions: u32 }` →
    `POST /channels/{c}/messages/{before}/ack` body `{manual: true, mention_count}` (A§5);
    `before` = the loaded message just before, else `message − 1 ms` as a snowflake.
    Threads: only when already joined and not archived (no join/unarchive writes).
  - `message_link(guild: Option<Snowflake>, channel, message) -> String` =
    `https://discord.com/channels/{guild|@me}/{channel}/{message}` (from `urls.rs`).
- Bar look: `surface_high` background, `border_normal`, `shadow_md`, radius control,
  28px pitch, 20px icons, 1px × 20px divider (H1–H5). Shift tracked through
  `on_modifiers_changed`; the expanded state resets when the row stops being hovered or
  the window deactivates.
- Context menu gains, in A§3's group order: Forward, Bookmark/Remove bookmark,
  "Remind me" (submenu, Task 5), Mark Unread, Copy Message Link.

- [ ] **Step 1: Failing tests:** `buttons` for own/other × normal/expanded (exact order);
  `message_link` for guild and DM; MarkUnread packet body; `before` choice.
- [ ] **Step 2–4:** run FAIL, implement, run PASS.

### Task 5: Reminders and bookmarks in the UI

**Files:**
- Create: `emerald-ui/src/app/panels/inbox/saved.rs`, `emerald-ui/src/app/reminder_modal.rs`,
  `emerald-ui/src/app/state/remind.rs`
- Modify: `panels/inbox.rs` (tabs), `chat/actions/menu.rs` (Remind me submenu),
  `app/alerts.rs` (due notification)

**Interfaces:**
- Produces:
  - `state::remind::RemindAt { In30m, In1h, In4h, NineAm, Monday9, Custom }` and
    `remind_at(choice, now: Local) -> i64` (Unix secs). `NineAm` reads
    "Today at 9:00" before 9:00, else "Tomorrow at 9:00"; Monday 9:00 = the next
    Monday. Labels from A§1's table (en/pt exact).
  - Inbox tabs, in order: Unreads, Mentions, **Bookmarks** ("Favoritos"), **Reminders**
    ("Lembretes", with an overdue count). Flat lists, no sections (A§1). Rows reuse the
    Unreads message look; actions: Jump, Remove (`UnsaveMessage`), and for reminders
    Edit/Snooze (opens the submenu) and Mark Complete (= DELETE). Bookmarks page by 25 with
    "Load more". Empty states with Discord's strings.
  - Due reminders: a timer at `Saved::next_due`; `take_due` → desktop notification
    "⏰ Reminder Due" / "⏰ Lembrete chegando" through `Alerts`, click opens the message.
  - Custom: a modal with a date and a time field (`TextInput`, parsed by
    `state::remind::parse_custom(date, time, now) -> Option<i64>`, rejects the past).

- [ ] **Step 1: Failing tests:** `remind_at` around 8:59/9:01 and on a Sunday/Monday;
  `parse_custom` valid/past/garbage; overdue count.
- [ ] **Step 2–4:** run FAIL, implement, run PASS.
- [ ] **Step 5:** Remove "Favorites and Reminders are not there yet" from README.

### Task 6: Identity data from the API

**Files:**
- Modify: `emerald-discord/src/model/user.rs`, `model/guild.rs`, `model/profile.rs`

**Interfaces:**
- Produces (B§1–B§7), all `#[serde(default)]` and lenient (unknown ids kept as numbers,
  never failing the parent):
  - `User.primary_guild: Option<PrimaryGuild { identity_guild_id, identity_enabled, tag,
    badge }>` and `PrimaryGuild::shown() -> bool` (enabled, guild id and tag set) and
    `badge_url(size) -> Option<String>` = `{CDN}/clan-badges/{guild}/{badge}.png?size=`
    with sizes 16/24/32/40 per B§1's map.
  - `User.display_name_styles` and `Member.display_name_styles: Option<NameStyle {
    font_id: u32, effect_id: u32, colors: Vec<u32> }>`.
  - `Role.icon: Option<String>`, `Role.unicode_emoji: Option<String>`,
    `role_icon_url(role, hash, px) -> String` (B§6).
  - `Relationship.since: Option<String>`.
  - `UserProfileDetails.theme_colors: Option<[u32; 2]>`, `Profile.widgets: Vec<Widget>`
    (`Widget { kind: String, data: serde_json::Value }` kept raw), `Profile.mutual_friends:
    Option<Vec<User>>`.

- [ ] **Step 1: Failing tests:** each field parses from a B-sample JSON; unknown font id
  999 parses; a `primary_guild` with `identity_enabled: false` is not `shown()`; a
  profile with an unknown widget type still parses.
- [ ] **Step 2–4:** run FAIL, implement, run PASS. Update `people`/`spaces` stores so the
  fields reach the UI (members keep their `display_name_styles`; roles keep icons).

### Task 7: Names 2026: server tags, role icons, display-name fonts

**Files:**
- Create: `emerald-ui/src/app/widgets/name.rs`, `emerald-ui/src/theme/name_fonts.rs`
- Create: `emerald-ui/assets/fonts/names/` (OFL fonts) + their licences
- Modify: `chat/list.rs` (header), `members/row.rs`, `sidebar/mod.rs` (DM rows),
  `friends/list.rs`

**Interfaces:**
- Produces:
  - `widgets::name::tag_chip(tag: &str, badge: Option<ImageSource>, badge_px: f32) -> Div`
    — about 18px tall, radius `tag`, `theme::hover()`-tinted background, 12px semibold;
    tooltip "Server Tag: {tag}" / "Tag do servidor: {tag}".
  - `widgets::name::role_icon(...)` 20px in message headers, 16px in member group
    headers; only when the guild has `ROLE_ICONS` (B§6); highest role with an icon.
  - `theme::name_fonts::family(font_id) -> Option<&'static str>`: ids 1–16 → the OFL
    families listed in B§2 that have a confirmed OFL licence (download from Google Fonts,
    regular weight only, add their `OFL.txt`); ids without one (Neo Castel, Ribes,
    Hexagon, unless confirmed OFL) → `None` = default font. Fonts load at start with the
    others.
  - Effects: gpui draws one text color, so every effect uses the first color pushed to
    contrast 3 (2.5 for gradient/glow/prism) against the background (B§2 step 1); a
    function `name_color(style, background) -> Option<u32>`. Where effects apply follows
    B§2 "Where shown": in guilds the role color and only the font.
- Show the tag chip after the name in message headers, member rows, DM rows, friend rows,
  the popout and the profiles.

- [ ] **Step 1: Failing tests:** `name_color` contrast reaches the target for a dark color
  on a dark background; `family` maps 4 → "Chicle" and 999 → `None`.
- [ ] **Step 2–4:** run FAIL, implement, run PASS.

### Task 8: Profile theme colors

**Files:**
- Create: `emerald-ui/src/app/state/profile_theme.rs`
- Modify: `profile.rs` (`Card` gains `theme: Option<ProfileTheme>`)

**Interfaces:**
- Produces `profile_theme(primary: u32, accent: u32) -> ProfileTheme { dark: bool,
  primary: u32, accent: u32, overlay: (u32, f32), text: u32, base: u32, button: u32 }`
  per B§3: `dark` from `1 − (0.299R+0.587G+0.114B)/255 > 0.5` on the primary; overlay
  black/white α0.6; text/base per `Zk` (hue 200–300 rule); button stepped until its
  contrast with white is 7–10. No `theme_colors` → `None` (plain card, as today).
  The card is a vertical gradient primary → accent with the overlay on top (inferred,
  B§3). gpui has `linear_gradient`; use it.

- [ ] **Step 1: Failing tests:** dark vs light for `0x202020`/`0xf0f0f0`; text contrast
  ≥ 4.5 against `base` for both; button contrast with white within 7–10.
- [ ] **Step 2–4:** run FAIL, implement, run PASS.

### Task 9: User popout

**Files:**
- Create: `emerald-ui/src/app/profile/popout.rs`
- Modify: `emerald-discord/src/rest/users.rs` (`type=popout` query),
  `chat/list.rs` (name and avatar click), `members/row.rs`, `profile.rs`

**Interfaces:**
- Produces:
  - `Command::FetchProfile` gains `kind: ProfileKind { Popout, Modal, Panel }`; popout query
    `type=popout&with_mutual_guilds=true&with_mutual_friends=true&guild_id=…` (B§4),
    not refetched within 60 s (the `profiles.request` cache).
  - `Emerald::open_popout(user, guild, anchor: Anchor, cx)`; anchor `left_of`/right of
    the clicked name or avatar (`Tracked`), clamped by the window. Clicking the name (M4)
    and the avatar (M5) opens it; "View Full Profile" stays in its ⋯ menu.
  - Card 300px (B§4): banner 105 with Add Friend / friend-state and ⋯ round buttons,
    avatar 80 ring 6, name with font, username + tag chip, badges in a bordered pill with
    "+N" over 6, mutual line (3 stacked friend avatars, "N Mutual Friends • N Mutual
    Servers"), bio, theme colors (Task 8), and the "Message @name" box at the bottom.
  - The box sends through the existing DM flow: `Command::OpenDm` then a normal send with
    nonce once the DM exists (B§4). If the user already has a DM, send directly. It is a
    real write: the user tests it by hand.
  - The own avatar keeps its own popover (`profile/own.rs`).

- [ ] **Step 1: Failing tests:** popout query string; badges overflow (`7 → 6 shown, "+1"`);
  mutual line text for 0/1/many.
- [ ] **Step 2–4:** run FAIL, implement, run PASS.

### Task 10: Full profile modal, two columns

**Files:**
- Modify: `profile/modal.rs`, `profile/social.rs`; Create: `profile/widgets.rs`
- Modify: `emerald-discord` (`Command::Games { ids }` → `GET /games?game_ids=` in batches
  of 20, B§5; `model::Game`)

**Interfaces:**
- Layout (gap section 5, B§8): about 960×800 (shrinks to the window), radius modal, theme
  colors. Left card 400 wide: banner 140, avatar 120, name, username + tag + badges pill,
  action row (Message — themed button; Add Friend; ⋯), bio with no heading, sections with
  12px semibold headings: "Member Since", "Friends Since" (`Relationship.since`, short
  date), "Note (only visible to you)" as an inline italic placeholder that becomes an
  input on click. Right: tabs "Board · N Mutual Friends · N Mutual Servers" ("Mural",
  2px underline). Activity tab is left out (outbox shape not found, B§5).
- Board shows the widgets it can draw (favorite game, games I play, games I like) from
  `Profile.widgets` with covers from `/games` (`app-icons/{id}/{cover}.webp`), cached in a
  bounded map. Unknown widget types are skipped.
- Close: round 32px button with `icons::CLOSE`.

- [ ] **Step 1: Failing tests:** `Game` parse; widget → game id extraction from the B§5
  sample; batching of 45 ids → 3 requests.
- [ ] **Step 2–4:** run FAIL, implement, run PASS.

### Task 11: DM profile panel 2026

**Files:**
- Modify: `profile.rs` (`render_profile_panel`)

Theme colors, banner 110 with the two round buttons, badges pill, tag chip, the mutual
line, bio without heading, "Member Since" with a 12px heading, pronouns on the username
line, the custom status as a bubble, no left rule, column 320. Reuses Tasks 7–9 pieces;
no new logic, no new tests beyond keeping the suite green.

- [ ] **Step 1:** Implement. **Step 2:** `cargo test -p emerald-ui` — PASS.

### Task 12: Member list Activity section

**Files:**
- Create: `emerald-discord/src/model/content_inventory.rs`, `rest`/`commands` entries,
  `emerald-ui/src/app/state/activity_feed.rs`, `emerald-ui/src/app/members/activity.rs`

**Interfaces (bundle-plan2 §4):**
- `Command::ContentInventory { refresh: Option<String> }` →
  `GET /content-inventory/users/@me?for_game_profile=false&feature=inbox` (+`refresh_token`)
  → `NetEvent::ContentInventory(Result<Feed, String>)`; gateway
  `CONTENT_INVENTORY_INBOX_STALE {refresh_after_ms}` schedules a refetch.
- `ActivityFeed`: fetch after READY and on window focus only when stale
  (`wait_ms_until_next_fetch`), not while idle > 15 min, backoff 1/2/4/8 min on failure.
- `ActivityFeed::for_guild(members, can_view) -> Vec<&Entry>`: types PLAYED_GAME,
  WATCHED_MEDIA, TOP_GAME, TOP_ARTIST, LISTENED_SESSION, LAUNCHED_ACTIVITY; authors who are
  members able to see the channel; unknown types skipped.
- Section "Activity" above the role groups, 3 cards and "show all", not in forums, only
  when the guild has under 10 000 members or the `ACTIVITY_FEED_ENABLED_BY_USER` feature.

- [ ] **Step 1: Failing tests:** feed parse with an unknown content type; `for_guild`
  filter; staleness and backoff timings.
- [ ] **Step 2–4:** run FAIL, implement, run PASS.

### Task 13: Look at it, README, final checks

- [ ] **Step 1:** Release build with a temporary `EMERALD_X_TEST` hook opening: a popout,
  the full profile, the forward modal (no send), the Inbox Bookmarks and Reminders tabs,
  a member list with Activity. Capture with `capture2.ps1`; fix what looks wrong. At most
  4 launches.
- [ ] **Step 2:** Remove the hook; `grep -rn "DEBUG-HOOK\|EMERALD_X_TEST" emerald-*/src`
  is empty.
- [ ] **Step 3:** README "What works" / "Not there yet" (Activity tab, editing widgets,
  name effects beyond one color, fonts not shipped).
- [ ] **Step 4:** `cargo build`, `cargo test`, `cargo clippy` with no new warnings.
- [ ] **Step 5:** Hand the user the list of real writes to try by hand: bookmark, reminder
  (and its notification), forward with text, mark unread, popout message box.
