# CLAUDE.md

Emerald is a native Discord client written in Rust with gpui. It signs in to a **real user
account** and talks to Discord's private API by imitating the official web client. Read
`README.md` for what already works, the architecture and the files the app writes. This
file covers how to work on the project and what to build next, in order.

## Rules for testing — read before running anything

Every launch and every request comes from a real Discord account. Discord bans accounts
it suspects of using third-party clients or automation, often permanently. The test
account must survive the whole project, so treat it as fragile.

**Never do these:**
- Send messages, reactions, uploads, posts or threads to other people or to public
  servers (VALORANT, English and the like) as a test. Do not join or leave threads, edit
  or delete messages, or pin anything there either.
- Send friend requests, join servers, open DMs with strangers, or accept invites as a
  test.
- Script or loop any action: no bulk sends, no repeated clicks, no scraping, no paging
  through history or member lists beyond what a person would scroll.
- Launch the app in a tight loop. Each launch is a new gateway IDENTIFY. Discord caps
  them per day and notices bursts. Keep it to a handful of launches per task, leave time
  between them, and quit the old instance first (`pkill -f "target/debug/emerald"`) so
  two sessions never run at once.
- Retry when Discord answers with a captcha, a 429, a 401 or an unusual error. Stop and
  tell the user.
- Print, log, commit or send the token (`~/.config/emerald/token` in debug builds,
  Keychain/Credential Manager in release). Never send the user's e-mail to any service.

**Do instead:**
- Prove behavior with unit tests first. Stores in `src/ui/app/state/` are UI-free and
  take fakes (`Outbox`, `MessageView`), so most logic can be tested with no network.
- To see UI, add a temporary local debug hook behind an env var, for example
  `EMERALD_X_TEST`, that opens the screen or injects fake data. Mark it
  `// DEBUG-HOOK-START` … `// DEBUG-HOOK-END`, and **remove it before finishing**. Check
  with `grep -rn DEBUG src` that nothing is left.
- Take screenshots in the background without stealing focus: get the window id of the
  app's PID with a small Swift script using CGWindowList, then run
  `screencapture -x -o -l<id> out.png`. Do not bring the app to the front; the user is
  working in other apps.
- When a feature cannot be checked without a real write (sending, posting, joining),
  stop. Say what was verified locally and what was not, and let the user test it
  themselves. If the user explicitly allows one real write, send it only to their own DM
  or own test server, once.
- Read-only use is fine in moderation: opening channels, scrolling a little, and viewing
  forums, profiles and member lists, as a person would.

## How to work here

- **Imitate the web client.** Before adding a request, find how discord.com does it:
  route, body, query string, headers, `location` values, batching and pagination. The
  client's scripts are downloaded into `target/discord-intl/` by
  `tools/discord_locales.py`; grep them there. When a value cannot be found, say that it
  is a guess, in a comment and to the user.
- **Respect the REST layer.** Requests go through `Call` in `src/discord/rest/`, with
  rate-limit buckets, `major` ids and the microcache for GETs. Writes that can trigger a
  captcha use `with_captcha!` in `commands.rs`.
- **UI ↔ network** only through `Command` and `NetEvent` (`src/discord/protocol.rs`).
- **Text:** every interface string goes in both `locales/en-US.toml` and
  `locales/pt-BR.toml`, written by hand; tests check this. Locale files generated from
  Discord's translations are not committed.
- **Language:** code, comments, logs, commits and docs are in English. Logs never
  contain the token or message content. Talk to the user in Portuguese.
- **Style:** match the surrounding code: doc comments on items, comments that explain
  why, no abbreviations in names.
- **Checks before saying "done":** `cargo build`, `cargo test` and `cargo clippy` with no
  new warnings. Update `README.md` ("What works" / "Not there yet") with each feature.
- **Git:** commit only when the user asks, and in separate commits when asked.
  Conventional prefixes (`feat:`, `fix:`, …). **Never** add `Co-Authored-By` or any
  Claude/Anthropic attribution.

## Roadmap

Work top to bottom. Each step is one feature or one commit. Within a phase, the order is
the suggested one; finish a phase before starting the next unless the user says
otherwise. Tick items off here as they land.

### Phase 0 — close what is open

Done but never checked in the real app, or known to be partial:

1. [x] Visual check of the thread rows in the sidebar (`w_auto` width fix), the thread
   chip under a message, and the ALT badge on images.
2. [x] Ask the user to try, by hand, one real upload, one spoiler image, one thread from
   a message, one forum post, and join/leave. Fix whatever they report.
3. [x] The `location` sent on joining or leaving a thread (`Thread Header`) is a guess;
   find the real value in the bundle.
4. [x] Files on forum posts (the post form has no attachments), which also unlocks
   posting in media channels.

### Phase 1 — the message box (daily-use pain first)

5. [x] Multi-line composer: Shift+Enter for a new line, the box grows up to a limit, then
   scrolls. `TextInput` is single-line today; edit mode needs it too.
6. [x] Drafts kept per channel when switching away.
7. [x] `@` autocomplete for members and roles (op 8 search is already there), `#` for
   channels, `:` for emoji, with arrow keys and Tab/Enter.
8. [x] Emoji picker button in the composer, reusing the reaction picker.
9. [x] Sticker and GIF pickers (GIFs come from Discord's Tenor proxy endpoints).
10. [x] Slash commands: listing and filling app commands. Treat with care, since
    interactions are bot traffic. Last in this phase.

### Phase 2 — reading messages fully

11. [x] Text spoilers `||x||`: hidden until clicked (they are only styled today).
12. [x] Missing markdown: block quotes (`>` and `>>>`), lists, subtext `-#`,
    timestamps `<t:…:R>`, channel and role mentions as pills, fenced code blocks with a
    block background, and masked links in embeds.
13. [x] Animated custom emoji and animated avatars (GIF/WebP frames, paused when the
    window is hidden).
14. [x] Video, audio and voice-message attachments (inline player, or at least a card
    that opens the file), and gifv embeds.
15. [x] Spoilers on files that are not images.
16. [x] Forwarded messages (`message_snapshots`) and polls (show the results, then vote).
17. [x] "New messages" bar, jump to the first unread, jump to the present, and click a
    reply to scroll to the original (fetching around it when not loaded).
18. [x] Open `discord.com/channels/…` message links inside the app.

### Phase 3 — getting around

19. [ ] Desktop notifications for DMs and mentions, honoring mute settings, plus an
    unread badge on the Dock / taskbar icon.
20. [ ] Mute and notification settings per server, channel and thread (read from and
    written to user guild settings, as the web client does).
21. [ ] Quick switcher (Cmd/Ctrl+K) across servers, channels and DMs.
22. [ ] Pinned messages panel per channel.
23. [ ] Search: messages in a server or DM with filters (from, in, has, before/after),
    results that jump to the message.
24. [ ] Threads browser: active and archived threads of a channel; forum sort, tag
    filter and search.
25. [ ] Inbox: recent mentions and unreads.
26. [ ] Server folders and order in the server list, as synced in user settings.
27. [ ] Mark channel/server as read; collapse state and hidden muted channels.

### Phase 4 — people and DMs

28. [ ] Group DMs: open, list members, name/icon, add/remove people, leave.
29. [ ] Close a DM; message requests and the spam inbox.
30. [ ] Block/unblock and ignore, with blocked messages collapsed.
31. [ ] Friend nicknames and user notes; mutual servers and friends in the profile.
32. [ ] Invites: create one for a channel; open invite links in messages as cards.
33. [ ] Leave a server.

### Phase 5 — your account

34. [ ] Profile editing: avatar, banner, bio, pronouns, accent color, per-server
    nickname and avatar.
35. [ ] Custom status (text, emoji, expiry) and activity display of others (games,
    Spotify).
36. [ ] Privacy settings: who can DM or friend you, message requests.
37. [ ] Devices/sessions list and logging other sessions out.
38. [ ] Multiple accounts, with fast switching. Mind the identify limits.

### Phase 6 — appearance and comfort

39. [ ] Light theme and following the system theme.
40. [ ] Zoom / UI scale, reduced motion, keyboard navigation and screen-reader labels.
41. [ ] Configurable shortcuts (mark read, next unread, Alt+Up/Down).
42. [ ] Window state remembered (size, position, last channel) and a tray / menu-bar
    item.

### Phase 7 — moderation (only what the user's roles allow)

43. [ ] Delete others' messages in bulk, kick, ban and timeout from the member list.
44. [ ] Channel and thread settings: name, topic, slow mode, lock/archive threads, forum
    tags.
45. [ ] Roles and permissions editor. Low priority.

### Phase 8 — voice and video (largest, last)

46. [ ] Join a voice channel: voice gateway, UDP, Opus encode/decode, input and output
    device choice, mute/deafen, speaking rings. Discord requires DAVE end-to-end
    encryption for voice now; plan for it from the start.
47. [ ] DM and group DM calls (ringing included).
48. [ ] Stage channels as a listener.
49. [ ] Video and screen share, viewing first, then sending.

### Phase 9 — shipping

50. [ ] Windows parity pass: build, login through WebView2, Credential Manager, paths,
    fonts and the IME.
51. [ ] macOS app bundle with icon, code signing and notarization; Windows installer
    and signing.
52. [ ] Auto-update.
53. [ ] `discord://` and invite deep links, and start on login.
54. [ ] Linux support (Wayland/X11, Secret Service for the token, WebKitGTK login). Only
    if the user wants it.

### Always

- Keep memory and CPU low: bound every new cache, and keep idle CPU at zero.
- Keep up with the web client: the build number is read live, but new required fields,
  headers or flows show up over time. When something breaks after a Discord update,
  check the current bundle before guessing.
