# DTM-16 v7 ("Discord Time Machine 2016") — design reference for Emerald's "2016" style

Source: `dtm16.slim.css` (build 1114, Oct 20 2026, by 11pixels). All values below were read from the
CSS; HSL tokens were converted to hex at the default `--saturation-factor: 1`. "a@N" means colour `a` at
alpha N. Where the theme does not override something, the value falls back to Discord's 2026 default;
that is marked "(not overridden)". Anything not read from the CSS is marked "(historical, not in CSS)".

---

## 1. Theme options (`@property` settings)

| Setting | Type / options | Default | Effect |
|---|---|---|---|
| `--dtm-16-plink` | `v2015` / `v2016` / `v2017` | **v2016** | Which UI year. Only 2016 is "accurate". 112 rules are 2017-only, 41 are 2015-only. |
| `--dtm-16-plink-ctb` | `v2016`, `v2017p1`, `v2017p2`, `v2017p3`, `v2017`, `v2019`, `v2020` | **v2016** | Composer ("chat textarea") version, separate from the UI year. |
| `--dtm-16-plink-cib` | `v2015` / `v2016` / `v2018` / `v2020` | **v2016** | Style of the "new messages" / "jump to present" bars. |
| `--dtm-16-ugly-mode` | checkbox | false | More literal but uglier versions of a few elements (square buttons in channel notices). |
| `--dtm-16-broken-modal-anim` | checkbox | false | Scale-in modal animation everywhere (fires twice sometimes). |
| `--dtm-16-2018-attach-menu` | checkbox | false | 2018-style attach popup menu. |
| `--dtm-16-hide-vc-panel-buttons` | `full` / `partial` / `none` | **none** | Hide the voice-panel action buttons (always / until hover / never). |
| `--dtm-16-titlebar-button-count` | integer 0-15 | **2** | Reserves `38px × n` on the right of the chat header for titlebar buttons that overlay it. |
| `--dtm-16-header-three-buttons` | checkbox | false | Deprecated, no effect. |
| `--dtm-16-hide-windows-titlebar` | checkbox | false | Removes the 22px Windows titlebar entirely. |
| `--dtm-16-outline-on-chat-avatar-focus` | checkbox | false | "2015 concept": 1px outline on a chat avatar on hover/focus instead of a shadow. |
| `--dtm-16-activity-feed-style` | `v1` / `v1-1` / `v2` | **v1** | Friends "Now playing"/activity column card style (2017-18, 2018-19, 2019-20). |
| `--dtm-16-old-game-profile` | checkbox | false | Marked "(DISABLED/BROKEN)". |
| `--dtm-16-oreo` | checkbox | false | Makes profile popouts and some menus light even in dark mode. |
| `--dtm-16-orp` | checkbox | **true** | "Old Rich Presence": activity card goes into the popout body with theme colours instead of a brand-coloured header. |

Discord's own "message group spacing" and "chat font size" settings are honoured:
group spacing 16 (default) → 20px gap between groups; 8 → 10px; 4 → 5px (no divider); 0 → 0; 24 → 25px.
Font size 12 → 36px avatars; 14/15/16 → 40px avatars. Content always starts 80px from the left.

---

## 2. Colour tokens

### 2a. Base palette (both themes)

| Token | Hex | Notes |
|---|---|---|
| blurple (brand) | **#7289da** | `--blurple`; hover **#677bc4**; active **#5b6eae**; 10% tint used for mentions |
| red | **#f04747** | hover #d84040; active #c03939 |
| green | **#43b581** | hover #3ca374; active #369167 |
| yellow | **#faa61a** | hover #e19517 |
| purple | #593695 | streaming |
| link | **#00b0f4** | `--text-link`, also the focus ring |
| HypeSquad / Partner / Bug-hunter badge | #f9a532 / #4087ed / #43b581 | |
| Spotify green (activity cards) | #1db954 | |

Presence colours are not overridden directly, but `--green-360`, `--red-400`, `--yellow-360` all point at
the palette above, so in practice: **online #43b581, idle #faa61a, dnd #f04747, offline #747f8d,
streaming #593695** (offline/streaming: historical values, consistent with the palette).

### 2b. Dark theme

Surfaces (2016 is three greys plus a darker rail; no 2026 "base-lowest/lower" split):

| 2016 token | Hex | Where |
|---|---|---|
| `--background-tertiary` | **#1e2124** | server list rail, scrollbar thumbs, panel button borders |
| `--background-secondary` | **#2e3136** | channel sidebar, member list, DM list, popout section bodies, panel button fill |
| `--background-secondary-alt` | **#282b30** | user panel and voice panel (bottom-left), context menu, selected-channel gradient |
| `--background-primary` | **#36393e** | chat column, header, settings content, modals, emoji picker |
| `--background-floating` | #18191c | |
| `--background-accent` | #4f545c | new-messages bar in sidebar, slider track, input borders |
| `--channeltextarea-background` | #40444b | (not used by the 2016 composer; see below) |

Text:

| Token | Hex |
|---|---|
| `--header-primary` | **#ffffff** |
| `--header-secondary` | #b9bbbe |
| `--text-normal` | **#dcddde** |
| `--text-muted` | **#72767d** |
| `--interactive-normal` | #b9bbbe |
| `--interactive-hover` | #dcddde |
| `--interactive-active` | #ffffff |
| `--interactive-muted` | #4f545c |
| `--channels-default` | #8e9297 (but channel rows are actually drawn as white at 30% opacity, see §5) |
| chat header "#" prefix | #8a8e94 |
| timestamps / "(edited)" | white@0.20 (≈ #5e6165 on #36393e) |
| day-divider text | #99aab5@0.5, line #474a4f |

Overlays and states:

| Token | Value |
|---|---|
| hover (`--background-modifier-hover`) | #4f545c@0.16 |
| active | #4f545c@0.24 |
| selected | #4f545c@0.32 |
| border/separator (`--background-modifier-accent`) | #ffffff@0.06 |
| message hover | #040405@0.07 |
| mentioned message bg / bar | #faa61a@0.10, left bar 4px #faa61a (bar backing #faa61a@0.20) |
| ephemeral / replying-to bg / bar | #7289da@0.10, bar #7289da |
| automod bg / bar | #f04747@0.10, bar #f04747 |
| group divider between message groups | #ffffff@0.04 (1px) |
| selected/hover channel/member/DM row | `linear-gradient(90deg, #282b30 85%, transparent)` plus a 2px blurple left bar (opacity 0.25 on hover, 1 when selected) |
| context menu | bg #282b30, item text white@0.6, hover bg #25282d / text #fff, separator rgba(245,245,245,.08), danger #f04747 (hover #ec1616) |
| classic popout (pins, inbox, search) | bg #2e3136, border #1c242b@0.6, header bg #35393e, shadow 0 2px 10px rgba(0,0,0,.2) |
| tooltip | **blurple #7289da** bg, white 12px/500 text, radius 3px |
| inline code | bg #2e3136, radius 3px; code block bg #2e3136, 2px border #282b30, radius 5px |
| embed | 4px left border #4f545c, inner grid bg #2e3036@0.3 with 1px #2e3036@0.6 border |
| invite embed / call tiles | bg #2e3136, 1px #2b2e33, radius 3px |
| reactions | bg white@0.06, mine blurple@0.30 with blurple count |
| mention pill | blurple@0.10 bg, blurple text; hover blurple@0.70 bg, white text |
| scrollbars | 14px wide, 3px transparent border → 8px thumb, radius 8px, thumb #1e2124, no track |
| voice call background | #161819 + a 200×204 webp pattern |

### 2c. Light theme

| Token | Hex |
|---|---|
| server rail (`--background-tertiary`) | **#e3e5e8** |
| channel sidebar / members (`--background-secondary`) | **#f2f3f5** |
| user panel (`--background-secondary-alt`) | **#ebedef** |
| chat (`--background-primary`) | **#ffffff** |
| floating | #ffffff |
| accent | #747f8d |
| header-primary / secondary | #060607 / #4f5660 |
| text-normal / muted | **#2e3338** / **#747f8d** |
| interactive normal / hover / active / muted | #4f5660 / #2e3338 / #060607 / #c7ccd1 |
| channels-default | #6a7480 (rows drawn at 50% opacity, muted 30%) |
| hover / active / selected | #747f8d@0.07 / @0.16 / @0.23 |
| separator | #060607@0.08 |
| selected row gradient | #ebebeb 85% → transparent |
| server pill | #2e3136 |
| chat header name / buttons | #4f545c / #99aab5; header shadow 0 2px 0 #eceeef@0.25 |
| timestamps | #99aab5; group divider #eceeef; day divider #99aab5 on #eceeef |
| composer | bg #ffffff, border 2px #dadddf, text #747f8d, placeholder #dadddf |
| context menu | bg #fff, item #99aab5, hover text #737f8d, hover bg #f9f9f9, separator #f5f5f5 |
| mentioned bg | #faa61a@0.20 |
| code block | bg #f9f9f9, border #f3f3f3 |
| popout section bg | #fbfbfb |
| scrollbar thumb in chat/channel list | **blurple** (light uses brand-coloured thumbs) |

### 2d. Mapping onto `emerald-ui/src/theme.rs`

| theme.rs fn | 2026 dark | **2016 dark** | **2016 light** | Note |
|---|---|---|---|---|
| `base_lowest` (rail + sidebar) | #2c2d32 | split: rail **#1e2124**, sidebar **#2e3136** | rail #e3e5e8, sidebar #f2f3f5 | 2016 needs a separate rail token |
| `base_lower` (chat) | #323339 | **#36393e** | #ffffff | |
| `base_low` (icon without image) | #36373e | #36393e (2016: placeholder icons on rail are #2e3136-ish) | #ffffff | |
| `surface_high` (composer, user panel) | #393a41 | composer **#42454a** (white@0.06 on #36393e); user panel **#282b30** | composer #ffffff; panel #ebedef | split into two tokens |
| `brand` | #5865f2 | **#7289da** | same | |
| `button_brand_hover` | #4452bb | **#677bc4** | same | |
| `danger` / `button_danger` | #f23f43 / #d22d39 | **#f04747** / hover #d84040 | same | |
| `online` / `button_positive` | #23a55a / #008545 | **#43b581** / hover #3ca374 | same | |
| `idle` | #f0b232 | **#faa61a** | same | |
| `offline` | #80848e | **#747f8d** | same | |
| `text_default` | #f3f3f4 | **#dcddde** | #2e3338 | |
| `text_strong` | #ffffff | #ffffff | #060607 | |
| `text_muted` | #abacb2 | **#72767d** | #747f8d | |
| `interactive` | #c5c6ca | #b9bbbe | #4f5660 | |
| `channels_default` | #999aa1 | #8e9297 (or white@0.3 → ≈ #6d6f72 on #2e3136) | #6a7480 (≈ #7c7c7e at 50%) | |
| `interactive_muted` | #4e5058 | #4f545c | #c7ccd1 | |
| `placeholder` | #6d6f78 | white@0.2 (≈ #686a6e) | #dadddf | |
| `link` | #76aff6 | **#00b0f4** | #00b0f4 | |
| `hover()` | #97979f@12% | #4f545c@16% | #747f8d@7% | |
| `active()` | @16% | #4f545c@24% | #747f8d@16% | |
| `selected()` | @20% | #4f545c@32% | #747f8d@23% | |
| `border()` | @12% | #ffffff@6% | #060607@8% | |
| `mention_text` / `mention_bg` | #c9cdfb / #5865f2@30% | **#7289da** / #7289da@10% | same | hover: white on blurple@70% |
| `mentioned` / `_hover` / `_bar` | #f0b232@10%/16% / #f0b232 | #faa61a@10% / @8% (core) / **#faa61a** | #faa61a@20% / @20% / #faa61a | DTM uses 10% dark, 20% light |
| `jumped` | #5865f2@20% | #7289da@20% (assumed; not in CSS) | | |
| `reaction_mine` | #5865f2@15% | **#7289da@30%** | same | |
| `code_bg` | #1e1f22@60% | **#2e3136** | #f9f9f9 | |
| `spoiler_hidden` | #1e1f22 | not overridden | | |
| `scrim` | black@72% | not overridden (historical 2016: black@85%) | | |

New tokens the 2016 style needs: `rail` (#1e2124), `panel` (#282b30), `composer_border` (white@0.2),
`timestamp` (white@0.2), `group_divider` (white@0.04), `selected_row_gradient` (#282b30→transparent),
`tooltip_bg` (blurple), `context_menu_bg` (#282b30), `server_pill` (#8a8e94 dark / #2e3136 light),
`popout_header` (blurple).

---

## 3. Typography

- **Family:** `Whitney, "gg sans", "Helvetica Neue", Helvetica, Arial, sans-serif` for both primary and
  display. **No font files are embedded** (no `@font-face` in the CSS); Whitney only works if installed,
  otherwise it falls to gg sans (served by Discord's own client, not by the theme). Emerald will need its
  own choice (Whitney is commercial; a close free stand-in, or keep gg sans/system).
- **Code:** `Consolas, "Liberation Mono", Menlo, Courier, monospace` (theme.rs's MONO already matches).
- Weights used are mostly **400 and 500**; 600/700 only for eyebrows ("ABOUT ME", "ROLES") and titles.

| Element | Size / weight / line-height | Colour |
|---|---|---|
| Message text | not overridden (Discord default 16px / 1.375) | #dcddde |
| Username in message header | not overridden (16px / 500, role colour) | role colour or #fff |
| Timestamp | not overridden size (≈12px); colour white@0.2 | |
| Channel name in sidebar | **15px / 500** | header-primary at 30% opacity (50% light); 100% when hover/selected/unread |
| Category name | **12px / 500 / 12px** | header-primary at 30%, uppercase |
| Server name (sidebar header) | weight **500** (size not overridden) | |
| Chat header channel name | **20px / 700 / 22px**, "#" prefix in #8a8e94 weight 300 | #fff / #4f545c |
| DM list name | **14px / 400 / 17.5px**; subtext 10px / 500 / 12px #99aab5 | |
| DM section header | 12px / 500 / 12px | |
| Member list group header | **12px / 500 / 12px uppercase**, white@0.4 | #99aab5 light |
| Member activity card name | 12px (14px when selected) | #fff |
| User panel name / tag | **14px / 400 / 16px** / **10px / 400 / 12px at 40% opacity** | header-primary |
| Day divider | 14px / 500 | #99aab5@0.5 |
| "NEW MESSAGES" divider label | 14px | red@0.8 |
| Badges (unread counts) | 12px / 500 / 12px, padding 3px 5px, radius 3px | white on red |
| Buttons | **16px**, padding 10px 20px (≈38px tall) | |
| Context menu item | **13px / 500 / 16px**, padding 6px 9px | |
| Tooltip | **12px / 500 / 12px**, padding 5px 10px | white on blurple |
| Popout display name / tag | **16px / 400 / 19px** white / **14px / 400** white@0.6 | |
| Popout section eyebrows | **11px / 700 / 14px** uppercase ("ABOUT ME", "ROLES", "MUTUALS") | header-secondary |
| Settings sidebar item | **14px / 400 / 1.25em**, padding 9px 6px 9px 20px | white@0.3, active #fff |
| Settings sidebar first header | 10px / 400 / 12px muted | |
| Friends list name / discriminator / status | 16px/500/18px white@0.6 / 12px/500 at 60% / 14px/400/16px white@0.4 | |
| Friends section headers | 12px / 500 uppercase, white@0.3 | |
| Embeds | provider 12/400; author 14/600/14; title 14/500/19; description 14/400/16; field name 14/600; value 14/500/1.1em | |
| Bot tag | 10px / 500 / 12px, padding 1px 2px, radius 3px, blurple bg | |
| Header tab pills (Friends "All/Online/…", expression picker tabs) | 14px / 18px line, padding 2px 8px, radius 4px, min-width 56px | white@0.6; active: white bg, #36393e text |

---

## 4. Shape

- **Avatars: round** everywhere (chat 40px, popout 90px inside a 96px ring, member list 30px, user panel
  30px, DM list 30px). No squircles.
- **Server icons: 50px circles (radius 25px); the selected server becomes a rounded square (radius 15px).**
  Hover does not change the shape. The change animates over 1.5s with a springy curve
  (`server-anim`: 25 → 11.8 → 16.1 → 14.7 → 15.3 → … 15px). Folder background radius 16px top / 25px bottom.
- **Server pill:** 12px wide fully rounded, offset −6px (only a 6px half-pill shows), colour #8a8e94 dark /
  #2e3136 light; min-height 12px (unread dot), height otherwise follows Discord's hover/selected states.
- **Radii:** buttons **3px**; inputs/composer **3px**; tooltips 3px; badges 3px; images/attachments **3px**;
  embeds 4px left bar; code blocks 5px; context menus **5px** (items 5px); user popout **3px**;
  profile modal 8px; settings modal 5px; expression picker 5px; generic modals 3px; channel rows,
  member rows, DM rows **0px** (flat, full width).
- **Shadows:**
  - guild (server name) header: `0 1px 0 #24272b, inset 0 -1px 0 rgba(0,0,0,.1)`
  - chat header: 1px bottom border #303337 + `0 1px 0 rgba(0,0,0,.06)`
  - composer form: `border-top 1px white@0.06` + `0 -1px 0 rgba(0,0,0,.1)`
  - popouts: `0 2px 10px rgba(0,0,0,.2), 0 0 0 1px #202225@0.6`
  - context menu: `0 0 1px rgba(0,0,0,.82), 0 1px 4px rgba(0,0,0,.1)` (light: `0 1px 4px rgba(0,0,0,.2)`)
  - badges: `0 1px 0 rgba(0,0,0,.25), inset 0 1px 0 white@0.15`
  - sidebar "unread above/below" bar: `0 2px 6px rgba(0,0,0,.24)`
  - elevation-high (pickers): `0 8px 16px rgba(0,0,0,.24)` + 1px stroke rgba(4,4,5,.15)
- **Borders/separators:** panel tops 1px #1e2124; panel button groups 1px #1e2124 with radius 4px and
  inner 1px dividers; header separators 1px × 24px rgba(216,216,216,.1); server-list separator 2px #2e3136.

---

## 5. Layout per area (2016 vs. Emerald's current 2026 look)

Emerald 2026 reference sizes (from `emerald-ui/src/app`): rail 72px with 40-48px icons; channel sidebar
240px; header ≈48px; user panel 56px (floating card); chat avatar 40px with 72px gutter; members 240px,
rows 44px.

### Window chrome
- Titlebar overlays the top 56px (macOS) or is a **22px system bar** on Windows with a "DISCORD"
  wordmark mask; back/forward buttons and the title are hidden. Inbox/help buttons (30px tall, 24px icons
  at 60% opacity) sit in the top-right over the chat header, which reserves `38px × count` (default 76px)
  on its right.

### Server list (rail)
- **80px wide** (2026: 72), bg #1e2124. Top padding 18px (+20/32px platform).
- Icons **50×50 circles**, selected → 15px radius; 2px separator #2e3136.
- Home button: Discord logo SVG on a 50px circle, 28px glyph.
- "Add server"/"Discover": 50px transparent circles with a **1px dashed** border in interactive-muted,
  turning interactive-normal on hover.
- Mention badge: red, radius 3px, padding 3px 5px, bottom-right offset −2px. Other badges (speaker,
  paused) on 24px #1e2124 circles at −6px.
- "Unread mentions" bars at top/bottom: 36-40px, radius 4px.
- "Download apps" button at the bottom with a 10px caption.

### Channel sidebar
- **240px**, bg #2e3136, no rounded corners, no gap from rail.
- Server header **56px**, padding 0 24px, name weight 500, shadow above; hover bg rgba(0,0,0,.1);
  banner gets a 58px top gradient rgba(0,0,0,.4)→transparent; boost/"guide" extras hidden.
- Channel rows: **flat, radius 0, padding-left 20px, right 6px**; text 15px/500 white at **30% opacity**
  (muted 10%; light 50%/30%); hover/selected: opacity 100% + gradient bg #282b30→transparent + 2px
  blurple left bar (25% hover, 100% selected). The channel type icon is replaced by a literal "#" in
  #8a8e94 with 3px gap; voice icon replaced by a 2016 speaker glyph.
- Unread: a **12px circle** at left −6px (half visible).
- Threads indented 40px with an 18px glyph; thread spine 4px wide at left 24px.
- Categories 12px/500 uppercase at 30% opacity, padding-left 20px, caret hidden until hover.
- Voice users in channels: white@0.5, speaking #fff; "LIVE" chip radius 0.
- User-limit chip: 16px tall, radius 4px, white@0.08 bg, 11px/500.
- Sidebar "new unreads" bar: 20px tall, 11px text, bg #4f545c (red for mentions), radius 3px.
- Scrollbar 14px / 8px thumb.

### User panel (bottom-left)
- **Full sidebar width (240px), flush, not a floating card.** Height **71px**, bg #282b30, top border
  1px #1e2124, padding 0 10px.
- Avatar **30px**, gap 10px; name 14px/400; second line 10px at 40% opacity, pulled up 3px.
- Mute / deafen / settings buttons form **one segmented group**: 1px #1e2124 border, radius 4px, each
  button bg #2e3136 with 20×20 2016 glyphs in #5d6063 (hover #b3b4b6), 1px dividers, no chevrons.
  Active-press: bg #34373c with inset shadow.
- Under it a **26px link bar** "Support • Change Log • Download Apps" (10px/500, #535559, bg #282b30,
  1px top border #2e3136, padding 6px 10px).

### Voice panel (connected)
- Same flush style: bg #282b30, 1px #1e2124 top, padding 10px.
- Status 14px/500/16px; channel subtext 10px/500/12px muted.
- Ping indicator: a **12×12 block** filled red (4px) / yellow (8px) / green (full) over #4b4d51.
- Disconnect etc.: segmented group like the user panel. Action buttons row 34px tall, 24px icons,
  colour white@0.4 → #fff.
- Voice user avatars 24px circles with 3px gap, collapsed after 6.

### Chat header
- **57px** tall (56 + 1px border #303337), bg #36393e, padding 0 (20 + reserved) 0 20px.
- "#name" 20px/700, "#" in #8a8e94 weight 300; the channel-type icon is hidden.
- Toolbar icons 30×30 hit area, 24px glyphs at **60% opacity** → 100% on hover, 8px apart; selected
  gets white@0.2 bg; separators 1×24px.
- Search box **144px → 244px** when focused, 28px tall, radius 3px, bg white@0.1, 500-weight
  placeholder white@0.3; filter tags 21px tall, #34383d.

### Message list
- Cozy only. Content padding-left **80px**, right 6px; avatar **40px** at left 20px, margin-top −2px.
- **Groups are separated by 20px + 1px divider (white@0.04) + 20px** (group spacing 16). Divider spans
  left 20px → right 6px.
- No message-hover background (`background: none` on rows); highlight only for mentions/ephemeral/
  automod with a 4px left bar at x = −6px.
- Day divider: line #474a4f, label 14px/500 on chat bg, padding 0 10px, margin-left 20px.
  "NEW MESSAGES" line red@0.5, label red@0.8.
- Replies: 16px reply avatars; reply spine 39px wide × 8px at (38px, 9px).
- Blocked-message groups render as a card: bg #32353a, 1px #2e3136, radius 3px, 12px/500 header.
- "New messages"/"jump to present" bars: **blurple@0.9 strip 26px tall, margin 0 20px, radius 0 0 3px 3px**
  (sits under the header), 14px/400 white; secondary action 12px/300 white@0.6.
- Inline code bg #2e3136 radius 3px padding 0.2em; code blocks 2px border, radius 5px.
- Invite embeds 400px wide, padding 8px 10px, 50px guild icon radius 15px, 40px join button.

### Hover action bar on messages
- **No floating toolbar box.** Icons are **16×16, inline, 4px apart, at 40% opacity** (100% on hover),
  colour #fff (light #99aab5), sitting at the top-right of the row (top ≈3px, ≈23px on a group's first
  message, 43px when it has a reply) on a bg = chat bg with a soft `0 0 6px 4px` same-colour glow so it
  masks the text underneath. Separators and quick-reaction buttons hidden; 2016 glyphs.

### Composer
- Form: margin **0 20px**, 1px top border white@0.06 + `0 -1px 0 rgba(0,0,0,.1)`, no padding.
- Text box: **42px inner height (46px with borders)**, **2px border white@0.2**, bg white@0.06
  (#42454a), radius 3px, text white@0.7, placeholder white@0.2. Grows to min(172px, 100vh − 260px).
  4px-wide scrollbar.
- **Attach button on the left, inside the box: 45px wide, full height, separated by a 1px right border**
  (white@0.2), glyph 21×25 white@0.2 → #fff on hover, hover bg white@0.06, press white@0.2. Text starts
  10px after it.
- **Gift, GIF, sticker and app buttons are hidden.** Only the **emoji button** remains on the right:
  a sprite of emoji faces, grayscale at 15% opacity (50% light), full colour on hover. Right margin 6px.
- Autocomplete opens attached on top of the box (same 2px border, no bottom border), section headers
  12px on a white@0.2 strip, rows padded 10px → 12px on hover with rgba(0,0,0,.12).
- Reply bar sits above the box: 28px tall, margin-top −20px, radius 3px 3px 0 0.
- Attachment/sticker previews: 59px strips.

### Typing indicator
- **Below the text box**, absolutely at the bottom (bottom 3px) across the full width, no left padding;
  dots then text 4px apart; colour white@0.8 (light #737f8d), slow-mode text 13px/400.
  (2026: above the composer on the left.) The bottom gradient over messages is removed.

### Member list
- **240px**, bg #2e3136, no left border.
- Group header 12px/500 uppercase white@0.4, **padding-left 30px**.
- Rows: radius 0, padding 30px left / 16px right, avatar scaled 0.9375 (32 → 30px), hover/selected gradient
  like channels. Row height not overridden (Discord default ≈42-44px).
- Activity rows become **196×64 cards** (margin-left 22px, radius 3px, bg rgba(0,0,0,.1), 1px rgba(0,0,0,.1))
  with an 18px blurple (or Spotify green) header strip that grows to full height when selected; name
  12px white; art 32px radius 3px. Only the first badge shown.

### User popout (click name/avatar)
- **250px wide, radius 3px**, bg = blurple with the 2016 "profile banner" PNG pattern (270×213, bottom);
  shadow `0 2px 10px rgba(0,0,0,.2)` + 1px ring.
- Banner min 250×87.5. **Avatar 90px circle with a 4px ring in light blurple #8599df**, centred
  (96px box, margin −16px top, 16px below); status dot 20px.
- Name 16px/400 white, tag 14px white@0.6, both centred; custom status 14px white, emoji 20px.
- Below the header, sections on **#2e3136** (#fbfbfb light), padding 14px 10px, with eyebrows
  "ABOUT ME", "ROLES", "MUTUALS" 11px/700. Role pills radius 3px, padding 0 7px, role-colour tinted
  bg/border, name white.
- Activity card in the body (Old Rich Presence on): bg secondary, 12px/600 title, 12px details, 4px
  progress bar, blurple buttons.
- Action buttons 32px tall, outlined white@0.3 → @0.6 → #fff, radius 3px.
- Badge row and display-name trailing icons **hidden**. Footer "Message @x" input kept, flat.

### Profile modal (full profile)
- Radius 8px, bg #36393e, grid: 213px left column (128px avatar with padding 40px 48px 40px 40px),
  header buttons 30px tall, **tabs 55px tall**, body 600px wide with 28px/14px padding and 1px
  #1e2124 top border; all on #2e3136. Only the first badge group shown, at 24px. Close button 24px.

### Settings
- **A centred 900px modal, not full-screen**: margin 60px auto, radius 5px; sidebar **188px**, content
  **712px**; a **75px footer** (#282b30, 1px #1e2124) with a blurple "Done" button (padding 10px 20px,
  16px/500); keybind hint hidden. Open animation `settingsOpen` scale 0.8 → 1.
- Sidebar items flat, 14px/400, padding 9px 6px 9px 20px, white@0.3 → #fff; selected gets
  `inset 4px 0 blurple`, hover `inset 4px 0 blurple@0.3`; icons hidden; separators 20px wide; only the
  first section header shown (10px muted). First header 56px with the header shadow.
- Log out row becomes an icon tile (40×38, red on hover).
- Eyebrows 11px/700 #b9bbbe; switches use ring rgba(255,255,255,.2).

### Modals / image viewer
- Generic modals: radius 3px, bg #36393e, header with `0 2px 3px rgba(0,0,0,.2)`.
- Image/video viewer top bar: **40×40 buttons, no bg, 24px glyphs at 30% opacity → 100%**, a
  1×20px separator after the action group. Media radius 3px.
- Invite modal: 46px copy field, 1px #4f545c border, 24px blurple text, square blurple "Copy" button
  turning green with a check on success.
- Upload drop modal: three 2016 file-icon illustrations.

### Emoji / sticker / GIF pickers
- Combined picker **472px tall: 22px tab row + 434px body**, bg #36393e, elevation-high shadow.
  Tabs "GIFs / Stickers / Emoji" are the header pill tabs (14px, padding 2px 8px, radius 4px, active white
  bg / #36393e text).
- Emoji picker header with 1px #1e2124 bottom border; **category rail on #1e2124**, starting 56px down,
  with **2016 bitmap/SVG category icons** (recent, people, nature, food, activity, travel, objects,
  symbols, flags, custom); selected category gets a 4px blurple bar at left −8px. Category pills hidden;
  diversity selector's extra button hidden.
- Locked (Nitro) items: lock icon 12px at 30% opacity.
- Soundboard picker radius 5px.
- GIF result focus: `0 11px 22px 1px #040405@0.3`.

### DMs / home / friends
- DM sidebar search: **57px header**, inner button 32px tall, radius 4px, white@0.05 bg, 1px
  rgba(0,0,0,.28) border, inset shadow; a CSS-drawn magnifier.
- DM rows: radius 0, **42px tall**, name 14px/400 at 30% opacity; Friends/Nitro/Shop/Message-requests
  rows get **30px round icon tiles** (#4f545c, blurple@0.25 on hover, blurple when selected).
- DM header text 12px/500, padding-left 20px, 30% opacity.
- Friends page: header tabs as pills with 1px separators; "Add Friend" pill blurple (white when
  selected). Add-friend header: padding 16px 30px 8px, gradient bg, 16px/700 uppercase title, 14px/500
  description, input radius 3px with 1px rgba(0,0,0,.3) border. Table headers 12px/500 uppercase with
  1px bottom border; rows radius 3px, hover #414449 with `0 2px 1px rgba(0,0,0,.1)`; action buttons
  square radius 3px #2e3136 → #4f545c, accept → green, deny → red.
- Empty chat uses two 2016 Wumpus illustrations (187×39, bottom-left/right) from static.discord.com.

### Discover
- 56px header bar with pill tabs, bg chat colour; hero uses a 2016 discovery splash from
  static.discord.com (no gradient); cards **146px tall** on #1e2124, 48px round icon, 18px/500 name,
  14px/600 member count (online count hidden), description revealed on hover.

### Misc
- Context menus min-width 170px, radius 5px, no outer padding, items 13px/500 with 2px vertical gap.
- Quick switcher: bg #2e3136, big input #585b5f with `0 2px 5px rgba(0,0,0,.2)`, selected result #282b30.
- Keyboard-key chips: #72767d with `inset 0 -4px 0` dark shadow.

---

## 6. 2026-only features

| Feature | Treatment |
|---|---|
| Avatar decorations | **Kept**, resized to the avatar box (40px chat; scaled around the 90px popout avatar). |
| Nameplates (DM/member rows) | **Untouched**, except nameplate colours are blanked on the DM close button. |
| Profile effects | **Hidden** (`.profileEffects` display none). |
| Nitro profile themes / colours | Popout **restyled**: the background is forced to blurple plus the 2016 pattern; the display-name main colour is forced to #fff. Custom banner images still show. Whether custom theme colours leak anywhere else was not verified. |
| Display-name styles (fonts/gradients) | Untouched in chat; reset to plain white only in member activity cards and popouts. |
| Role icons | **Not referenced** (untouched). |
| Badges (profile) | **Hidden in the popout**; the modal shows only the first badge group at 24px. |
| Server/clan tags | **Restyled** to tiny chips: 10px muted text, bg white@0.06, radius 2px, padding 0 3px, max-width 36px, tag badge icon hidden. |
| Boost goal, server guide, header ellipse/glass effects | Hidden. |
| Quests / Nitro orbs lottie / upsells | Orbs lottie hidden; profile upsells hidden. |
| Super-reaction glow | Hidden (burst glow display none). |
| Gift / GIF / sticker / app buttons in the composer | **Hidden** (only emoji remains); the pickers are still reachable through the emoji button's tabbed picker. |
| Message hover toolbar with quick reactions | Replaced by 16px inline icons; separators hidden. |
| Activity feed, rich presence | Restyled to 2017-era cards (option-controlled). |

---

## 7. Embedded assets (dtm16.css)

MIME types: **270 `image/svg+xml` URL-encoded**, 24 `image/svg+xml;base64`, **17 `image/png;base64`**,
**5 `image/webp;base64`**. **No fonts.**

What matters for a native port:
- **~150 SVG icon masks** (mask/-webkit-mask) and **394 SVG `d: path(...)` replacements**: the 2016
  icon set (mute, deafen, settings, disconnect, video/screen-share call controls, channel "#"/speaker,
  header icons, message action icons, context-menu icons, thread glyph, unread arrows). All are vector
  path data and can be pulled out and redrawn as Emerald icons.
- **Emoji button sprite**: PNG ≈53 KB (grayscale) + colored PNG ≈85 KB.
- **Emoji picker category icons**: 10 SVGs (`--dtm-16-emoji-picker-category-*`).
- **File-type icons**: 13 SVGs (`--file-icon-video/acrobat/ae/sketch/ai/archive/code/document/ps/spreadsheet/webcode/audio/unknown`).
- **Profile popout banner pattern**: PNG ≈50 KB dark, ≈49 KB light; activity-card pattern PNG ≈20 KB each;
  small profile-banner background pattern PNG ≈2 KB.
- **Voice call background** (`--dtm-16-vc-bg-image`): WebP ≈16 KB, 200×204, dark only.
- Home button / download-apps / quick-switcher footer / streamer-mode / login sidebar (Discord logo with
  gradient): SVGs.
- Connection icons (League PNG ≈2 KB, Reddit PNG ≈4 KB, plus SVGs), upload-modal file illustrations,
  activity-feed/member-card WebP (≈3 KB), appearance-settings theme preview WebPs (≈140 KB each,
  one ≈540 KB).
- Remote (not embedded) images on static.discord.com: two Wumpus "empty chat" pairs, discovery
  splash (dark/light), and a few others.

---

## 8. Broken / unknown / caveats

- Message text, username and timestamp **sizes are not overridden**; they use Discord 2026 defaults with
  the Whitney stack. Historically 2016 cozy text was ~15px; that is not in the CSS.
- Server-pill heights for unread/hover/selected are not overridden (only width 12, min-height 12).
- Member and channel row heights are not set (Discord defaults).
- The CSS uses Chromium-only features: `@container style(...)` queries for every option, CSS `if()`
  in DM rows (my parser split those values, so DM-row 2017 branches are unreliable), `:has()`, and `d: path()`
  on SVG.
- "Old game profile" is flagged by the author as disabled/broken; "broken modal animation" is
  intentionally glitchy; titlebar buttons can overlap the header search when the button count is wrong.
- `--staus-warning-text` is misspelled in the source (so `--status-warning-text` keeps Discord's value).
- Whitney is not shipped; a native port must pick or license a font.
- Emerald's `jumped`, `spoiler_hidden` and `scrim` have no DTM equivalent; the values above are
  suggestions.
- Profile-theme colour handling beyond the popout background was not traced.
