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
- Prove behavior with unit tests first. Stores in `emerald-ui/src/app/state/` are UI-free and
  take fakes (`Outbox`, `MessageView`), so most logic can be tested with no network.
- To see UI, add a temporary local debug hook behind an env var, for example
  `EMERALD_X_TEST`, that opens the screen or injects fake data. Mark it
  `// DEBUG-HOOK-START` … `// DEBUG-HOOK-END`, and **remove it before finishing**. Check
  with `grep -rn DEBUG emerald-*/src` that nothing is left.
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

- **Crates.** The workspace is eight crates at the root (see the README's Architecture):
  `emerald-core` (paths, i18n, frames, processes), `emerald-http` (transport, caches,
  rate limits), `emerald-audio` (microphone, speakers, noise suppression; generic),
  `emerald-discord` (endpoints, gateway, models, the voice engine), `emerald-ui` (interface),
  `emerald-webview` (generic native webview, no Emerald code), `emerald-notify`
  (generic notifications and icon badge) and `emerald-bin` (the executable, the
  login/captcha flows, and what joins the UI to the platform). A crate depends only on the ones before it in
  that list. New request bodies and gateway frames go in `emerald-discord/src/packets/`;
  Discord hosts come from `urls.rs`, never as literal URLs.
- **Small modules.** Keep files to one concern, roughly under 400 lines. When an area
  grows, make it a folder of modules (`rest/`, `commands/`, `text_input/`) rather than
  a longer file; share repeated logic through a helper or trait (`or_expired`,
  `token::Store`, `gateway::dispatch::decode`).
- **Imitate the web client.** Before adding a request, find how discord.com does it:
  route, body, query string, headers, `location` values, batching and pagination. The
  client's scripts are downloaded into `target/discord-intl/` by
  `tools/discord_locales.py`; grep them there. When a value cannot be found, say that it
  is a guess, in a comment and to the user.
- **Respect the REST layer.** Requests go through `Call` (`emerald-http`), with
  rate-limit buckets, `major` ids and the microcache for GETs. Writes that can trigger a
  captcha use `with_captcha!` in `commands.rs`.
- **UI ↔ network** only through `Command` and `NetEvent`
  (`emerald-discord/src/protocol.rs`).
- **Text:** every interface string goes in both `emerald-core/locales/en-US.toml` and
  `emerald-core/locales/pt-BR.toml`, written by hand; tests check this. Locale files generated from
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

19. [x] Desktop notifications for DMs and mentions, honoring mute settings, plus an
    unread badge on the Dock / taskbar icon.
20. [x] Mute and notification settings per server, channel and thread (read from and
    written to user guild settings, as the web client does). Threads still only
    inherit from their channel: their own mute lives on the thread membership.
21. [x] Quick switcher (Cmd/Ctrl+K) across servers, channels and DMs.
22. [x] Pinned messages panel per channel.
23. [x] Search: messages in a server or DM with filters (from, in, has, before/after),
    results that jump to the message.
24. [x] Threads browser: active and archived threads of a channel; forum sort, tag
    filter and search.
25. [x] Inbox: recent mentions and unreads.
26. [x] Server folders and order in the server list, as synced in user settings.
27. [x] Mark channel/server as read; collapse state and hidden muted channels.

### Phase 4 — people and DMs

28. [x] Group DMs: open, list members, name/icon, add/remove people, leave.
29. [x] Close a DM; message requests and the spam inbox.
30. [x] Block/unblock and ignore, with blocked messages collapsed.
31. [x] Friend nicknames and user notes; mutual servers and friends in the profile.
32. [x] Invites: create one for a channel; open invite links in messages as cards.
33. [x] Leave a server.

### Phase 5 — your account

34. [x] Profile editing: avatar, banner, bio, pronouns, accent color, per-server
    nickname and avatar.
35. [x] Custom status (text, emoji, expiry) and activity display of others (games,
    Spotify).
36. [x] Privacy settings: who can DM or friend you, message requests.
37. [x] Devices/sessions list and logging other sessions out.
38. [x] Multiple accounts, with fast switching. Mind the identify limits.

### Phase 6 — appearance and comfort

39. [x] Light theme and following the system theme.
40. [x] Zoom / UI scale, reduced motion, keyboard navigation and screen-reader labels.
    Partial: zoom (90% to 150%, Cmd/Ctrl with + and -) and reduced motion are done. Tab
    focus and screen-reader labels wait for gpui to have focusable controls and an
    accessibility tree.
41. [x] Configurable shortcuts (mark read, next unread, Alt+Up/Down).
42. [x] Window state remembered (size, position, last channel) and a tray / menu-bar
    item. The menu bar item is macOS only; Windows waits for the parity pass (phase 9).

### Phase 7 — moderation (only what the user's roles allow)

43. [x] Delete others' messages in bulk, kick, ban and timeout from the member list.
44. [x] Channel and thread settings: name, topic, slow mode, lock/archive threads, forum
    tags. Partial: forum tags can be added and removed, not renamed.
45. [ ] Roles and permissions editor. Low priority; not started. Permissions are read for
    the checks above, not edited.

### Phase 8 — voice and video (largest, last)

46. [x] Join a voice channel: voice gateway, WebRTC, Opus encode/decode, input and output
    device choice, mute/deafen, speaking rings. Discord requires DAVE end-to-end
    encryption for voice now; plan for it from the start. Done over WebRTC as the web
    client does (str0m, pure-Rust crypto, Chrome's offer lines) with davey for DAVE.
    Not yet checked in a real call. Missing: echo cancellation, push-to-talk,
    per-user volume.
47. [x] DM and group DM calls (ringing included). Rings through `POST call/ring` once
    CALL_CREATE arrives, as the web client does; no ringtone sound yet.
48. [x] Stage channels as a listener. Asking to speak is not there.
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
