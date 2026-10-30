# Visual styles: a faithful 2026 look, then 2018 and 2016

Status: design for **project 1** (faithful 2026 + style foundation + custom title bar).
Projects 2 (2018) and 3 (2016) get their own design documents later; their reference
material is already collected next to this file.

## Why

The user wants Emerald to offer three visual styles in Settings → Appearance: **2026**
(the default), **2018** and **2016**. The two old styles recreate the BetterDiscord themes
DTM-18 and DTM-16 (`themes/dtm-18.theme.css`, `themes/dtm-16.theme.css`), including their
layout, not only their colors. Everything that exists in 2026 must also exist in the old
styles, redrawn the way that era would have drawn it (avatar decorations, nameplates, role
badges, pickers, and so on).

Today's UI is, in the user's words, "a mix of 2018 and 2026". So the order the user chose
is:

1. **Make 2026 faithful** to Discord as it is now, using the user's screenshots in
   `themes/2026/` (each file name, in Portuguese, describes the feature shown). While doing
   it, move the look into a swappable *style* so 2018 and 2016 become mostly values plus
   layout variants instead of a rewrite.
2. **2018** (DTM-18) and the style selector.
3. **2016** (DTM-16), last.

## Decisions already made by the user

- Three styles: 2026 (default), 2018, 2016. Order of work: 2026 → 2018 → 2016.
- Old styles copy DTM-16 / DTM-18 layout too, not just palette.
- Dark / light / system stays a separate choice and applies to every style.
- **Left out for now:** Nitro, Shop (Loja), Quests (Missões) and every Nitro upsell or ad.
  They are "the delicate part of Discord"; may be revisited later, not in this project.
- **Custom window decoration** drawn by the app in all three styles, like Discord does,
  instead of the operating system's title bar.
- **Font:** a free (open-licence) look-alike font, bundled with the app. Discord's own
  fonts (gg sans in 2026, Whitney in 2016/2018) are licensed and must not be shipped.

## Goals (project 1)

1. **Style foundation.** All colors, radii, key sizes and fonts the UI uses come from the
   active style. Only the 2026 style exists at the end of project 1; there is no selector
   yet (it arrives with 2018). Adding a style later must not require touching every screen
   for values; screens whose *layout* differs per era branch on the style explicitly.
2. **Custom title bar** (2026 version): back/forward history arrows on the left, the
   current page's icon + title centered, Inbox and Help on the right, then window controls
   drawn by the app (minimize, maximize/restore, close) on Windows. Inbox moves here from
   the chat header. No "download app" button.
3. **Missing screens and components**, built the 2026 way:
   - Image / GIF viewer: dark full-window overlay; author avatar, name and time top-left;
     pill toolbar top-right (zoom, forward, download, open in browser, more) and a separate
     close button. Opens from attachments and embed media.
   - User popout on clicking a name or avatar in chat, members and DMs; "View full
     profile" opens the full profile.
   - Microphone and headset split buttons in the user panel, each with a chevron opening a
     menu: input/output device list (two-line rows), input profile, volume slider, and
     "Voice settings".
   - Discover (servers only): compass button under "+" in the server list, sidebar with
     "Servers" (and "Apps" only if it is read-only browsing), category tabs, search, hero,
     and featured-server cards. Read-only endpoints, imitated from the web client.
   - Full guild header: banner image, invite button, boost goal bar, and the Server Guide /
     Events / Channels & Roles rows when the server has them.
   - Home "Active Now" column and the friends search field.
4. **Old layouts redone the 2026 way:**
   - Settings as a centered rounded modal over a scrim, with header bar, plain X close,
     sidebar with user header, search, icons per item and sentence-case groups; the Account
     page as label/value rows with bordered "Edit" buttons.
   - Full profile modal: two columns (~960px), action buttons, tabs, "friends since",
     inline note. DM side profile panel matched to the screenshot.
   - Profile editor: three columns (editor, live preview, widgets/"Mural"), without the
     Nitro upsell.
   - Composer: one bar (~56px) with "+" inside on the left and GIF, Sticker, Emoji (and
     Apps, if app commands are usable there) on the right; no Gift button.
   - Expression pickers: ~495×485, server/category rail on the left, collapsible
     icon+name headers, larger cells, search with icon and focus ring; locked emoji and
     stickers shown greyed with a lock instead of hidden (no Nitro upsell banner); animated
     GIF previews; GIF landing grid. Label the GIF provider as the web client currently does.
   - Typing indicator above the composer with animated dots; composer bottom aligned with
     the user panel.
   - Friends: brand-blue "Add Friend", round icon actions on rows, sentence-case counts,
     tabs as in the screenshot, Add Friend form as one combined box with the optional
     message and counter.
   - Chat header: "•" separator, notification bell, search box with border and icon on the
     right, button order as in the screenshot.
   - Hover action bar on messages: forward button, divider after quick reactions, Edit (not
     Reply) on own messages, Shift expands it, raised background with shadow.
5. **Everything the screenshots show is drawn and works** (user's decision), including
   features Emerald does not have yet: forward message (hover bar and image viewer), the
   Shift-expanded hover actions (save/bookmark, mark unread and the rest shown), edit alt
   text and remove attachment on own images, the optional message on friend requests, GIF
   favorites, the "Message @name" box in the popout, microphone input profiles (voice
   isolation / studio; needs new processing in `emerald-audio`), profile widgets (Mural,
   Activity, game collection, add widget), Phone / Password / two-factor rows on the
   Account page, and a drag-resizable sidebar. Each is researched in the web client bundle
   first. Each one that writes to Discord cannot be tested by the builder: verify locally,
   then hand it to the user to try by hand (CLAUDE.md rules). Password, phone and two-factor
   are sensitive account flows: they must follow the web client exactly, including its MFA
   and captcha steps, and never log anything from them.
6. **Nitro-only profile features in the editor** (avatar decoration, profile effect,
   frame, display-name style, profile theme colors): the user has no Nitro. In the 2026
   style they are **shown but locked** (visible, not editable, with a lock), with no upsell
   text or button. In 2018 and 2016 they are not shown in the editor. Other people's Nitro
   cosmetics are still displayed everywhere.
7. **New data from the API**, displayed everywhere 2026 shows it: role icons (image or
   unicode emoji) next to names and member-list group headers; server tags (primary guild
   / clan chips); Nitro profile theme colors; display-name styles (font/effect/gradient)
   and gradient role colors where the API exposes them; embed footer icon and timestamp.
8. **Details:** "21:22" instead of "Hoje às 21:22" for today's messages, hover time in the
   gutter of follow-up messages, Twemoji images for unicode emoji, light-theme fix for
   white text on brand/danger/positive buttons, member list without the top search box
   and with the activity section, and the other items in the gap reports.

The full list, with file:line references into the current code and pixel measurements
from the screenshots, is in the four gap reports:

- [gap-shell.md](2026-10-29-visual-styles/gap-shell.md): title bar, server list, sidebars,
  user panel, friends, add friend, Discover, chat header, member list, typing.
- [gap-messages.md](2026-10-29-visual-styles/gap-messages.md): message rows, hover bar,
  popouts, full profile, DM profile panel, decorations and badges.
- [gap-media.md](2026-10-29-visual-styles/gap-media.md): image viewer, composer, pickers,
  embeds.
- [gap-settings.md](2026-10-29-visual-styles/gap-settings.md): settings modal, profile
  editor, microphone and headset menus.

Where the gap reports list something excluded above (Nitro/Shop/Quests rows, Gift button,
Nitro upsells, the download button), the exclusion wins.

## Non-goals

- The 2018 and 2016 styles and the style selector (projects 2 and 3).
- Nitro, Shop, Quests, upsells, ads, gift button. (Nitro editor features appear locked in
  2026 only, see goal 6.)
- Server Discovery actions that write (joining from Discover follows the normal join flow,
  which already exists; nothing new that writes).
- Roles and permissions editor, video and screen share (separate roadmap items).

## Anti-goals (would make this a failure even if it works)

- Any new network behavior that looks unlike the web client, or any extra request burst
  (Discover, role icons, profile data must use the same routes, query strings and
  pagination as discord.com, and only when the user opens that screen).
- Hard-coding values in screens again, so 2018/2016 would mean touching every file.
- Idle CPU above zero (animated dots, animated GIF previews and the title bar must not
  redraw when nothing changes or the window is hidden) or unbounded caches (Twemoji,
  Discover banners, GIF previews).
- Shipping a licensed font.

## Constraints

- All rules in `CLAUDE.md` apply, especially testing on a real account: no test writes,
  few launches, debug hooks behind env vars removed before finishing, unit tests first.
- Every new interface string in both `en-US.toml` and `pt-BR.toml`.
- Files about one concern, under ~400 lines.
- UI ↔ network only through `Command` / `NetEvent`; new endpoints in `emerald-discord`,
  hosts from `urls.rs`, requests through `Call`.
- `cargo build`, `cargo test`, `cargo clippy` clean; README "What works" updated per step.
- One feature per commit, conventional prefixes, commit only when the user asks.

## Calls made while writing this (check these)

- **Nothing that works today is dropped.** When the 2026 layout has no place for an
  Emerald feature (member-list search, Blocked tab, in-place DM filter, extra settings pages
  such as Accounts, Shortcuts, Devices, Desktop, explicit Save/Reset in the profile
  editor), it moves to where Discord 2026 keeps the equivalent (for example the blocked list
  in settings), or stays as an extra item styled the 2026 way.
- **Dark mode values** come from the web client's `.theme-dark` CSS tokens (the default
  "Dark" variant), like `theme.rs` already does; the screenshots are all light. Ash, Onyx
  and the other dark variants are not in scope.
- **Windows is the primary check target** (the user runs Windows and the screenshots are
  Windows); macOS must keep building and behave sensibly. Screenshots for checking need a
  Windows background-capture method that does not steal focus (CLAUDE.md's is macOS-only).
- **Done means** every gap-report row is closed, excluded, or listed in README "Not there
  yet". Work in order of visual impact (the ranked lists at the end of each gap report),
  style foundation first. 2018 starts after project 1.

## Left to the builder

- **Font choice** (builder's call, marked as such): pick an open-licence (OFL or similar)
  sans whose shapes are close to gg sans, compare it against the screenshots, and bundle it
  with its licence file. Keep the system font as the fallback. 2016/2018 may later get a
  second font closer to Whitney.
- **Shape of the style foundation:** how the active style is stored and read (the current
  `theme.rs` uses free functions over an atomic; extending that pattern is fine).
- **Title bar mechanics** per platform (gpui's transparent title bar and window-control
  areas on Windows; on macOS, keep the native traffic lights inside the custom bar, as
  Discord does on Mac; this is a guess, not something the user stated).
- How back/forward history is defined (suggested: navigation between channels/screens).
- Order of the steps inside project 1, as long as the style foundation comes first.
- Where Twemoji images come from (bundled vs fetched from Discord's asset host the way the
  web client does) as long as the cache is bounded.
- Exact endpoints for Discover, role icons and profile theme colors, found in the web
  client bundle (`target/discord-intl/`); guesses must be said to be guesses.

## Reference for later projects

- [dtm18-reference.md](2026-10-29-visual-styles/dtm18-reference.md): DTM-18 palette, shape,
  layout per area and treatment of 2026-only features. DTM-18 defines only the dark palette;
  the light values in the report are a proposal.
- [dtm16-reference.md](2026-10-29-visual-styles/dtm16-reference.md): DTM-16 v7 palette
  (dark and light), shape, layout per area, its options and defaults, and the ~150 vector
  icons it embeds.
