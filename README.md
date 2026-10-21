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
  covered (and are not downloaded) until clicked; alt text shows on hover.
- **Message actions** from a hover toolbar or right click: reply (with or without the
  ping), edit in place (Up edits your last message), pin, copy, delete with confirmation
  (Shift skips it).
- **Reactions**: counts that follow the gateway live, super reactions shown apart, a click
  on a chip to add or take back yours, and Discord's picker from the hover toolbar, the
  context menu or the + after the chips: search, "Frequently Used" (whose first three also
  sit on the toolbar), the server's custom emoji (every server's with Nitro) and the Unicode
  categories.
- **Discord markdown**: bold, italics, underline, strikethrough, code, headings, mentions,
  clickable links, custom emoji drawn inline (large when a message is only emoji). Masked
  links always show their real host.
- **Member list** loaded on demand: even in servers with millions of members, only the
  visible pages are kept in memory.
- **Unread state** synced with other devices, with read receipts batched; mention counts
  on channels, DMs and servers, and messages that ping you highlighted.
- **Settings**: display name, status, font size, compact mode; language and 12/24-hour time.

Not there yet: voice, files on forum posts (so media channels can't be posted to), animated
custom emoji (shown still).

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

```
src/
├── main.rs              entry point, webview child modes, login cycle
├── i18n.rs              interface text per language, the `t!` macro
├── logging.rs           tracing to stderr and a daily log file, panics included
├── paths.rs             per-platform config, cache and log directories
├── settings.rs          JSON preferences
├── webview/             login and captcha webview (child process)
│   ├── mod.rs           shared scripts and flows
│   ├── macos.rs         WKWebView backend
│   └── webview2.rs      WebView2 backend (Windows)
├── discord/             everything that talks to Discord, no UI
│   ├── protocol.rs      Command, NetEvent, Snapshot: the UI ↔ network contract
│   ├── commands.rs      runs each Command against the REST API
│   ├── gateway.rs       zlib-stream WebSocket, resume, heartbeat, ops 8 and 37
│   ├── rest/            HTTP client: `Call` builder, RateLimiter, microcache, typed errors
│   ├── model/           Discord payloads, only the fields in use
│   ├── permissions.rs   role and overwrite permission algorithm
│   ├── identity.rs      headers and super properties matching the web client
│   ├── build.rs         current web client build number, read from discord.com/app
│   ├── token.rs         token storage: Keychain, Credential Manager or file
│   └── cache.rs         SQLite disk cache
└── ui/
    ├── markdown.rs      Discord markdown parser
    ├── text_input.rs    text field
    ├── theme.rs, icons.rs, time.rs
    └── app/             the main window
        ├── app.rs       `Emerald`: composes the stores and lays out the window
        ├── state/       UI-free stores, tested in isolation
        ├── events.rs    hands each NetEvent to the stores
        ├── navigation.rs, composer.rs
        └── sidebar.rs, chat/, members.rs, friends.rs, profile.rs, settings.rs, …
```

## Translations

Interface text lives in `locales/<code>.toml`, one file per language, with Discord's
locale codes (`en-US`, `pt-BR`, `de`, …). Code reads it with `t!("friends.add")`, or
`t!("composer.dm", name = name)` for text with arguments. `build.rs` embeds every file in
`locales/`, so adding a language means adding a file. A key a language lacks falls back to
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
