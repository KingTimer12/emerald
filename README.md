# Emerald

A native Discord client for macOS and Windows, written in Rust with [gpui](https://www.gpui.rs/).
The interface follows the look of the official web client (in Brazilian Portuguese),
and the app uses a fraction of an Electron client's memory: roughly 60 to 100 MB, with
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
- **Settings**: display name, status, font size, compact mode.

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
