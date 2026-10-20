# Emerald

A native Discord client for macOS and Windows, written in Rust with [gpui](https://www.gpui.rs/).
The interface follows the look and wording of the official web client, in English or
Brazilian Portuguese, and the app uses a fraction of an Electron client's memory: roughly 60 to 100 MB, with
CPU near zero when idle.

> **Warning.** Third-party clients on user accounts violate Discord's Terms of Service
> and can get the account suspended. Emerald mimics the official web client and stays
> within the rate limits, but the risk is yours. Use it at your own discretion.

## What works

- **Login** through Discord's own sign-in page in a native webview (WKWebView on macOS,
  WebView2 on Windows), captcha included. The token is kept in the config directory
  (see [Files](#files)); logging out revokes it on the server.
- **Friends**: Online, All and Pending tabs; add by username, accept, decline, remove.
- **Direct messages**: list ordered by activity, search, history, sending with optimistic
  confirmation, typing indicators, a profile panel (badges, bio, connected accounts)
  and the full profile.
- **Servers**: channels and collapsible categories, permissions computed the way Discord
  does (hidden channels, sending blocked without permission), nicknames and role colors,
  replies, images, embeds, stickers and system messages.
- **Discord markdown**: bold, italics, underline, strikethrough, code, headings, mentions,
  clickable links. Masked links always show their real host.
- **Member list** loaded on demand: even in servers with millions of members, only the
  visible pages are kept in memory.
- **Unread state** synced with other devices, with read receipts batched.
- **Settings**: display name, status, font size, compact mode; language and 12/24-hour time.

Not there yet: voice, threads, forums, reactions, emoji images, editing messages.

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
| `DISCORD_TOKEN` | Use this token instead of the stored one. |
| `EMERALD_LOCALE` | Locale sent to Discord, when it should differ from the interface language. |
| `EMERALD_BUILD_NUMBER` | Override `client_build_number` when the one in the code goes stale. |
| `RUST_LOG=emerald=debug` | Verbose logs. |

Tests:

```sh
cargo test
```

## Files

| macOS | Windows | Contents |
|---|---|---|
| `~/.config/emerald/token` | `%APPDATA%\emerald\token` | Account token. Never share it. |
| `~/.config/emerald/settings.json` | `%APPDATA%\emerald\settings.json` | Preferences. |
| `~/Library/Caches/emerald/cdn` | `%LOCALAPPDATA%\emerald\cdn` | CDN image cache, capped at 200 MB. |
| | `%LOCALAPPDATA%\emerald\WebView2` | WebView2 browser profile; the login session itself is InPrivate. |

## Architecture

Two threads: the UI thread (gpui) and a network thread running a single-threaded tokio
runtime. They talk through messages: the UI sends `Command`s, the network answers with
`NetEvent`s.

```
src/
├── main.rs              entry point, webview child modes, login cycle
├── i18n.rs              interface text per language, the `t!` macro
├── paths.rs             per-platform config and cache directories
├── settings.rs          JSON preferences
├── webview/             login and captcha webview (child process)
│   ├── mod.rs           shared scripts and flows
│   ├── macos.rs         WKWebView backend
│   └── webview2.rs      WebView2 backend (Windows)
├── discord/             everything that talks to Discord, no UI
│   ├── protocol.rs      Command, NetEvent, Snapshot: the UI ↔ network contract
│   ├── commands.rs      runs each Command against the REST API
│   ├── gateway.rs       zlib-stream WebSocket, resume, heartbeat, ops 8 and 37
│   ├── rest/            HTTP client: `Call` builder, RateLimiter, typed errors
│   ├── model/           Discord payloads, only the fields in use
│   ├── permissions.rs   role and overwrite permission algorithm
│   ├── identity.rs      headers and super properties matching the web client
│   └── cache.rs         on-disk image cache
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
