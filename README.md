# Emerald

A native Discord client for macOS and Windows, written in Rust with [gpui](https://www.gpui.rs/).
The interface follows the look and wording of the official web client, in English or
Brazilian Portuguese, and the app uses a fraction of an Electron client's memory: roughly 60 to 100 MB, with
CPU near zero when idle.

> [!CAUTION]
> **Using Emerald can get your Discord account banned.** Emerald is not made or approved
> by Discord. It signs in to a normal user account and talks to Discord's private API,
> which Discord forbids. Discord's help center, on
> [automated user accounts](https://support.discord.com/hc/en-us/articles/115002192352-Automated-user-accounts-self-bots):
>
> > Automating normal user accounts (generally called "self-bots") outside of the OAuth2/bot
> > API is forbidden, and can result in an account termination if found.
>
> And the [Terms of Service](https://discord.com/terms) (effective September 29, 2025), under
> "Don't use the services to do harm to Discord", list among the forbidden uses:
>
> > using any unauthorized software designed to modify the services; [...] copying,
> > dismantling, or reverse engineering any of our services
>
> Emerald behaves like the official web client: the same requests and headers, the current
> build number, its rate limits and caching, and nothing automated. That lowers the odds of
> standing out but does not make it allowed or safe. Discord does not publish how it detects
> third-party clients, and a ban can be permanent. Only use an account you can afford to lose,
> and never script it (mass messages, friend requests, joining servers, scraping).

## What works

- **Login** through Discord's own sign-in page in a native webview (WKWebView on macOS,
  WebView2 on Windows), captcha included. The token is kept in the macOS Keychain or the Windows
  Credential Manager (see [Files](#files)), wiped from memory when dropped, and never
  logged; logging out revokes it on the server.
- **Friends**: Online, All and Pending tabs; add by username, accept, decline, remove.
- **Direct messages**: list ordered by activity, search, history, sending with optimistic
  confirmation, typing indicators, a profile panel (badges, bio, connected accounts)
  and the full profile.
- **Servers**: channels and collapsible categories, permissions computed the way Discord
  does (hidden channels, sending blocked without permission), nicknames and role colors
  that follow live member and role changes, replies, images, embeds, stickers (Discord's
  animated ones too) and system messages.
- **Threads and forums**: joined threads under their channel (and the open one), a chip
  on the message that started a thread, permissions derived from the parent the way
  Discord does; forums list their posts as cards with tags, the opening message, its
  image and the message count, paging in archived posts as you scroll. Start a thread
  from a message's menu (named after the message, as the web client suggests), write a
  forum post with a title, tags and its first message, and join or leave a thread from
  its header.
- **Uploads**: up to 10 files per message from the + button, dragged onto the chat or
  pasted, with image previews, alt text and spoilers; sent the way the web client sends
  them, with the size limit following Nitro and server boosts. Spoiler images arrive
  covered (and are not downloaded) until clicked; alt text shows on hover. Forum and
  media channel posts take files too.
- **Message box**: several lines (Shift+Enter), growing up to twelve rows and then
  scrolling; a draft kept per channel, with its reply and files, when you switch away.
  Suggestions as you type: `@` for members (asked of the gateway the way the web client
  does), roles, `@everyone` and `@here` when allowed; `#` for channels; `:` for emoji;
  picked with the arrows and Tab or Enter, and sent as real mentions.
- **Emoji, stickers and GIFs** from the button at the end of the message box: the emoji
  picker writes into the message; stickers (the server's, every server's with Nitro, and
  Discord's standard packs) and GIFs from Discord's Tenor search are sent at once, as on
  Discord. Stickers animate in the picker too, only those on screen.
- **Slash commands**: `/` lists the server's, DM's and your own apps' commands; options
  are written as `name:value` with a hint showing which are required; Enter runs it
  once. Option autocomplete and file options are not supported yet.
- **Message actions** from a hover toolbar or right click: reply (with or without the
  ping), edit in place (Up edits your last message), pin, copy, delete with confirmation
  (Shift skips it).
- **Reactions**: counts that follow the gateway live, super reactions shown apart, a click
  on a chip to add or take back yours, and Discord's picker from the hover toolbar, the
  context menu or the + after the chips: search, "Frequently Used" (whose first three also
  sit on the toolbar), the server's custom emoji (every server's with Nitro) and the Unicode
  categories.
- **Discord markdown**: bold, italics, underline, strikethrough, inline code and fenced
  code blocks, headings, subtext (`-#`), lists, block quotes (`>` and `>>>`), spoilers
  hidden until clicked, timestamps (`<t:…>`, every style, in your timezone), mentions as
  pills (roles in their color; a click on a user opens the profile, on a channel opens
  it), clickable links, custom emoji drawn inline (large when a message is only emoji,
  animated ones moving while the window has focus). Masked links always show their real
  host. `discord.com/channels/…` links show as a pill and open in the app, jumping to the
  message.
- **Media and embeds**: video attachments as their still with a play button, audio files
  and voice messages as cards (they open in the browser: there is no player yet), file
  spoilers covered like image ones; embeds with fields, thumbnails and images, image
  links and GIFs, GIF attachments included (their frames decoded off the UI thread, GIF
  and animated WebP alike, playing while focused); forwarded messages with where they
  came from; polls with live results and voting, and a "Create Poll" form from the + menu.
- **Moving through history**: a "new messages" divider and bar (jump to the first unread,
  or mark as read), clicking a reply or a message link loads the page around the original
  and flashes it, and a bar leads back to the present when reading older messages.
- **Animated avatars** on profiles and the user panel, while the window has focus.
- **Member list** loaded on demand: even in servers with millions of members, only the
  visible pages are kept in memory.
- **Unread state** synced with other devices, with read receipts batched; mention counts
  on channels, DMs and servers, and messages that ping you highlighted. "Mark As Read"
  on a channel, DM or server from its right-click menu.
- **Notifications**: desktop notifications for new messages, following each place's
  settings as the web client does (muted places and `@silent` messages stay quiet,
  "Only @mentions" with @everyone and role pings suppressible, nothing in Do Not
  Disturb, nothing for the channel on screen); clicking one opens its channel (Windows).
  Unread mentions and friend requests as a count on the taskbar button or Dock icon.
- **Mute and notification settings** per server, category, channel and DM from the
  right-click menu: mute for 15 minutes up to "until I turn it back on", All / Only
  @mentions / Nothing / the category's default, suppress @everyone and role pings,
  hide muted channels. Synced with Discord both ways; muted channels show dimmed and
  without unread marks. Folded categories are synced too.
- **Server folders** and order as arranged on any device: folders show their first four
  servers, open into a column of their color, and carry the unread dot and mentions.
- **Quick switcher** (Ctrl+K / Cmd+K): channels, threads, DMs and servers by name, the
  recent ones first; `#`, `@` and `*` narrow to channels, people or servers.
- **Pinned messages**, **search** and **inbox** panels from the channel header. Search
  takes Discord's filters (`from:`, `mentions:`, `in:`, `has:`, `before:`, `during:`,
  `after:`, `pinned:`), in a server or a DM; the inbox lists recent mentions and the
  unread channels. A result jumps to its message.
- **Threads browser**: a channel's active and older threads, and a search by name.
  Forums sort by activity or by date, filter by tags and search posts by name.
- **Settings**: display name, status, font size, compact mode; language and 12/24-hour time.

Not there yet: voice; playing video and audio inside the app; animated avatars in the
chat list (still there, as Discord shows them until hovered). On macOS, notifications go
through `osascript` (Notification Center only takes bundled apps), so clicking one does
not open its channel; search dates are taken in UTC.

## Requirements

- macOS or Windows 10/11.
- A recent stable Rust toolchain (edition 2024).
- macOS: no full Xcode install needed, Metal shaders are compiled at runtime
  (`gpui` with `runtime_shaders`). Login uses WebKit and AppKit.
- Windows: the MSVC toolchain (Visual Studio Build Tools with the Windows SDK). Login
  uses the Edge WebView2 runtime, which ships with Windows 11 and current Windows 10.
  Release builds are GUI apps with no console window.

## Running

```sh
cargo run --release
```

The first run opens Discord's login window. To use a token you already have:

```sh
DISCORD_TOKEN=... cargo run --release
```

Useful environment variables:

| Variable | Purpose |
|---|---|
| `DISCORD_TOKEN` | Use this token instead of the stored one. It is removed from the environment once read. |
| `EMERALD_LOCALE` | Locale sent to Discord, when it should differ from the interface language. |
| `EMERALD_BUILD_NUMBER` | Force a `client_build_number`. Normally it is read at startup from `discord.com/app`, as the web client loads it, and cached for offline starts. |
| `EMERALD_LOG=emerald=debug` | Log filter, in `tracing`'s directive syntax (`RUST_LOG` works too). The default is `warn,emerald=info`. |

Tests:

```sh
cargo test
```

## Files

| macOS | Windows | Contents |
|---|---|---|
| Keychain, item "Emerald" | Credential Manager, `Emerald:discord-token` | Account token (release builds). |
| `~/.config/emerald/token` | `%APPDATA%\emerald\token` | Account token in debug builds, mode 0600. A release build moves it to the Keychain or Credential Manager and deletes the file. |
| `~/.config/emerald/settings.json` | `%APPDATA%\emerald\settings.json` | Preferences. |
| `~/Library/Caches/emerald/cache.sqlite` | `%LOCALAPPDATA%\emerald\cache.sqlite` | SQLite cache, readable only by you: CDN images (capped at 200 MB, least recently used out first) and the last profiles fetched, cleared on sign-out. |
| `~/Library/Caches/emerald/build_number` | `%LOCALAPPDATA%\emerald\build_number` | Last web client build number seen. |
| | `HKCU\Software\Classes\AppUserModelId\Emerald.Desktop` | Registry key naming the app on its notifications. |
| `~/Library/Logs/Emerald/emerald.YYYY-MM-DD.log` | `%LOCALAPPDATA%\emerald\logs\…` | One log per day, the last 7 kept, in a folder only you can read. They start with the version, platform and folders in use; they never hold the token, and name people and channels only by id. |
| | `%LOCALAPPDATA%\emerald\WebView2` | WebView2 browser profile; the login session itself is InPrivate. |

## Architecture

Two threads: the UI thread (gpui) and a network thread running a single-threaded tokio
runtime. They talk through messages: the UI sends `Command`s, the network answers with
`NetEvent`s.

GET requests that opt in go through a [microcache](https://blog.nginx.org/blog/benefits-of-microcaching-nginx):
answers are reused for one second, identical requests in flight wait for the first one
instead of going out again, an expired copy is served while it refreshes, and when Discord
fails (network error, 5xx, rate limited) a recent copy stands in. Writing to a channel
drops what was cached for it. On a burst, Discord sees one request instead of several.

The code is a Cargo workspace of seven crates at the root. Each depends only on the
ones above it, and `emerald-webview` and `emerald-notify` on nothing of Emerald's. Large areas are folders
of small modules, one concern each:

```
emerald-core/            shared by every crate, no network and no UI
├── locales/             interface text, one TOML file per language
├── build.rs             embeds every file in locales/
└── src/
    ├── i18n.rs          interface text per language, the `t!` macro
    ├── paths.rs         per-platform config, cache and log directories
    ├── frames.rs        GIF and WebP frames, decoded and scaled off the UI thread
    └── process.rs       child runs of this executable (webviews) and restarts
emerald-http/            HTTP transport, nothing Discord-specific in its API
└── src/
    ├── base_url.rs      `BaseUrl`: a host written once, joined with each path
    ├── client.rs        `Client` under one BaseUrl, `Call` builder, `Session` headers
    ├── downloads.rs     whole-URL downloads (CDN, media) through the disk cache
    ├── ratelimit.rs     per-bucket and global limits, 429 handling, invalid-request guard
    ├── microcache.rs    one-second GET cache with request coalescing and stale copies
    ├── cache.rs         SQLite disk cache
    └── error.rs         typed failures and the captcha round trip
emerald-discord/         everything that talks to Discord, no UI
└── src/
    ├── lib.rs           `spawn`: the network thread; `sign_out`
    ├── protocol/        Command, NetEvent, Action: the UI ↔ network contract
    ├── commands/        runs each Command: one module per area, shared error handling
    │                    (`outcome.rs`) and the captcha retry (`captcha.rs`)
    ├── rest/            every REST endpoint as the web client calls it, by area
    │                    (messages, threads, users, guilds, apps, expressions)
    ├── gateway/         the reconnect loop, one connection, dispatch and zlib inflate
    ├── packets/         what goes on the wire: gateway frames, REST bodies, context headers
    ├── model/           Discord payloads, only the fields in use (message/ by part)
    ├── token/           `Token`, the `Store` trait (file, Keychain, Credential
    │                    Manager) and the `Vault` that picks one
    ├── urls.rs          Discord's hosts (web, API, CDN, media, gateway) as BaseUrls
    ├── permissions.rs   role and overwrite permission algorithm
    ├── identity.rs      headers and super properties matching the web client
    └── client_build.rs  current web client build number, read from discord.com/app
emerald-ui/              the gpui interface
├── assets/icons/        monochrome SVG icons, embedded
└── src/
    ├── settings.rs      JSON preferences
    ├── markdown.rs      Discord markdown parser (blocks, inline spans, links)
    ├── text_input.rs    text field (editing, offsets, layout, element, IME)
    ├── theme.rs, icons.rs, time.rs, emoji.rs, lottie.rs
    ├── app.rs           `Emerald`: composes the stores; the window is drawn in app/render.rs
    └── app/             the main window
        ├── state/       UI-free stores, tested in isolation
        ├── events.rs    hands each NetEvent to the stores (handlers in events/)
        ├── navigation.rs, composer.rs
        ├── alerts.rs    which messages notify, and the count on the app icon
        ├── panels/      pins, search, inbox and threads beside the chat
        ├── place_menu.rs  right-click menu: mark read, mute, notification levels
        ├── switcher.rs  the quick switcher (Ctrl/Cmd+K)
        └── sidebar/, chat/, members.rs, friends.rs, profile.rs, settings.rs, …
emerald-webview/         a blocking native webview window, generic: `WebView` builder,
└── src/                 a `Backend` per platform, posts back one string
    ├── lib.rs           `WebView`, `Page`, `POST_JS`
    ├── trust.rs         which pages may post back
    ├── macos.rs         WKWebView backend
    └── windows/         WebView2 backend: the Win32 window and the engine
emerald-notify/          desktop notifications and a count on the app icon, generic
└── src/
    ├── lib.rs           `Notifier`, `Notice`, clicked notifications as a channel
    ├── macos.rs         Dock badge, notifications through osascript
    └── windows/         toasts under a registered app id, taskbar overlay badge
emerald-bin/             the `emerald` executable
├── assets/              app icon, the Windows resource file, the login capture
│                        script and the captcha page
├── build.rs             embeds the icon into the Windows .exe
└── src/
    ├── main.rs          entry point, webview child modes, login cycle
    ├── login.rs         the login and captcha flows, on emerald-webview
    ├── alerts.rs        emerald-notify behind the interface's `Alerts`
    └── logging.rs       tracing to stderr and a daily log file, panics included
```

## Translations

Interface text lives in `emerald-core/locales/<code>.toml`, one file per language, with Discord's
locale codes (`en-US`, `pt-BR`, `de`, …). Code reads it with `t!("friends.add")`, or
`t!("composer.dm", name = name)` for text with arguments. `emerald-core/build.rs` embeds every
file in `locales/`, so adding a language means adding a file. A key a language lacks falls back to
English. The language follows the system until one is picked in Language & Time; switching
applies at once, and Discord's own answers (errors, for one) follow it too.

`en-US.toml` and `pt-BR.toml` are written by hand. The tests check that every key the code
uses exists, that pt-BR is complete, and that no language uses unknown keys or arguments.

Other languages can be filled from the Discord web client's own translations:

```sh
python3 tools/discord_locales.py            # every language Discord has
python3 tools/discord_locales.py --only de,fr
```

The script downloads the client's scripts once into `target/discord-intl` (about 100 MB),
pairs each language's message bundle with the English one, and keeps the messages whose
English and Portuguese text match ours. Keys with no Discord counterpart, such as
Emerald's own messages and date formats, are left to the English fallback. The wording
belongs to Discord, so check whether you may redistribute it before committing these files.

A few design choices:

- **UI-free stores.** `People`, `Spaces`, `History`, `ReadStates`, `Typing` and
  `MemberList` hold state and know nothing about drawing. The on-screen message list
  follows `History` through the `MessageView` trait, and the UI sends commands through
  the `Outbox` trait; tests use fakes for both.
- **Browser-like behavior.** Headers, nonces, read receipts, ops 8 and 37 and pagination
  follow what the web client does, so the client neither stands out nor abuses the API.
  REST requests respect rate-limit buckets, a global cap below Discord's, and a guard
  against too many invalid requests.
- **Bounded memory.** History for up to 8 channels with 300 messages each, up to 1500
  decoded images, and only the visible pages of the member list.
