# DTM-18 v3 design reference for Emerald's "2018" style

Source: `dtm18.slim.css` (3,287 lines, BetterDiscord theme by @dav154, v3, "Aims to recreate the 2018 Discord style").
All hex values below come from resolving the theme's HSL tokens at saturation factor 1. When the theme
leaves a value to Discord's own CSS, this report says so and labels the suggested value
**[not in DTM, 2018 reference, unverified]**.

Emerald baseline (2026), from `emerald-ui/src` and the screenshots: server list 72px wide, icons 40px
with 12px radius, unread pill 4×8/20/40px; channel header 48px; floating, rounded user panel 56px tall;
message avatar 40px with 72px left inset; member rows 44px with 32px avatars; reactions 28px tall;
profile side panel 340px; the screenshots use the light theme.

---

## 1. Theme options and defaults

DTM-18 has **no `@property` / settings UI**. (DTM-16, in the same folder, has some; DTM-18 has none.) Everything it varies on comes from Discord's own settings or from root variables:

| Knob | Values | Default / effect |
|---|---|---|
| `--colorblind-status` | `1` | Defined on `:root`, **never read** anywhere. The status glyph SVGs (`--colorblind-idle/dnd/offline/streaming/unknown`) are also defined and never used. So nothing happens in v3. The intent was 2018's accessibility glyphs drawn inside the status dot: idle = clock hand, dnd = horizontal bar, offline = small circle, streaming = play triangle `#bdafd5`, unknown = "?". The glyphs are `#2f3136` on dark and `#f6f6f7` on light, in a 10×10 viewBox. |
| Message group spacing (Discord setting → `--f-msg-spacing`) | 0→20px, 4→25px, 8→30px, **16 (Discord default)→40px**, 24→50px | Half goes above a group and half below: the default is 20px + 20px, with a 1px separator line between groups (see §5). |
| Chat font size (Discord setting) | 12→72%, 14→82%, 15→92%, **16→100%**, 18→110%, 20→125%, 24→150% | Scales the root font size; every `rem` below scales with it. |
| Cozy / compact | both handled | Compact: avatar 1em at left 0, timestamp `width: calc(20px + 8ch)`, hanging indent 9ch. |
| `--f-msg-paddingLeft` | 80px (48px for system messages) | Left inset of message text. |
| `--custom-app-top-bar-height` | 0px; 22px on Windows | The Windows title bar is a 22px strip and each window button is 28px wide. |
| `--custom-guild-list-width` | 70px | Server list width. |
| `--custom-folder-color` | brand `#7289da` | Default folder tint. |
| Theme | `.theme-dark` only defines the palette | Light is only partly handled (§2, §7). darker/midnight only change the folder-preview opacity (0.5 instead of 0.25). |
| Locale | the en-US unread pill is rewritten to "NEW MESSAGES" | Other locales keep Discord's text. |
| `.full-motion` | adds a 0.2s transition on cards | — |

## 2. Color tokens

### 2.1 Raw legacy scales (resolved)

**primary** (neutral greys used for every dark surface):
100 `#f6f6f7` · 130 `#eff0f0` · 160 `#e6e6e8` · 200 `#dcddde` · 230 `#d1d3d5` · 260 `#c6c8ca` · 300 `#b9bbbe` · 330 `#a3a6aa` · 345 `#96989d` · 360 `#8e9297` · 400 `#72767d` · 430 `#686d73` · 460 `#5d6269` · 500 `#4f545c` · 530 `#484c54` · 560 `#40444b` · **600 `#36393f`** · **630 `#2f3136`** · 645 `#282a2e` · **660 `#292b2f`** · **700 `#202225`** · 730 `#1e1f22` · 760 `#1b1d20` · 800 `#18191c` · 830 `#121315` · 860 `#0c0d0e` · 900 `#040405`

**brand** (blurple): 100 `#f8f9fd` · 130 `#f2f4fc` · 160 `#ebeefa` (invalid in the CSS, see §7) · 200 `#e3e7f8` · 230 `#dae0f5` · 260 `#d1d9f3` · 300 `#c7d0f0` · 330 `#b5c1ec` · 360 `#a5b3e7` · 400 `#8ea2e1` · 430 `#8699df` · 460 `#7d92dd` · **500 `#7289da`** · 530 `#687cc6` · 560 `#5c6eb1` · 600 `#4e5d94` · 630 `#435180` · 660 `#3b4770` · 700 `#2e3757` · 730 `#2b3352` · 760 `#272f4b` · 800 `#222941` · 830 `#1a2032` · 860 `#111521` · 900 `#06070b`. Alpha variants `brand-05a`…`95a` are `#7289da` at 5%–95%.

**grey** (pre-2023 light-theme scale, defined but not used for surfaces): 100 `#f8f9f9` · 130 `#f2f3f5` · 160 `#ebedef` · 200 `#e3e5e8` · 230 `#dbdee1` · 260 `#d2d6da` · 300 `#c7ccd1` · 330 `#b6bcc4` · 345 `#acb2bb` · 360 `#a6adb6` · 400 `#9099a4` · 415 `#8a8e94` · 430 `#88919d` · 445 `#87909c` · 460 `#80848e` · 475 `#7f8996` · 500 `#747f8d` · 530 `#6a7480` · 560 `#5e6772` · 600 `#4f5660` · 630 `#444b53` · 660 `#3c4149` · 700 `#2e3338` · 730 `#2b2f35` · 760 `#282c31` · 800 `#23262a` · 830 `#1b1d20` · 860 `#111315` · 900 `#060607`

**semantic**: positive `#43b581` (hover `#3ca375`, active `#369166`) · warning `#faa81a` (canonical 2018 value `#faa61a`; the difference is HSL rounding; hover `#e19317`, active `#c88615`) · danger `#f04747` (hover `#d84040`, active `#c03939`). Each has 5%–95% alpha variants.

### 2.2 Resolved `--legacy-*` semantic tokens (dark; `.theme-dark` block)

| DTM token | Value | Composite on its usual surface |
|---|---|---|
| `legacy-bg-primary` | primary-600 `#36393f` | chat, header, settings content |
| `legacy-bg-secondary` | primary-630 `#2f3136` | channel sidebar, member list, settings sidebar |
| `legacy-bg-secondary-alt` | primary-660 `#292b2f` | user panel, code-block border, layer-modal content |
| `legacy-bg-tertiary` | primary-700 `#202225` | server list, layer-modal backdrop |
| `legacy-mod-light-light / medium / heavy / solid` | `#4f545c` at 30% / 48% / 60% / 100% | on `#2f3136`: `#393c41` / `#3e4248` / `#42464d` |
| `legacy-mod-dark-light / medium / heavy / solid` | `#202225` at 30% / 48% / 60% / 100% | on `#36393f`: `#2f3237` / `#2b2e33` / `#292b2f` |
| `legacy-input-bg` | black 10% | `#313339` on chat |
| `legacy-input-border-default` | black 30% | `#26282c` |
| `legacy-input-border-interact` (hover) | `#040405` | — |
| `legacy-bg-mentioned` | warning 10% | `#4a443b` |
| `legacy-icon-interactive-normal / hover / active` | white 60% / 80% / 100% | on `#2f3136`: `#acadaf` / `#d5d6d7` / `#ffffff` |
| `legacy-text-primary` | `#f6f6f7` | — |
| `legacy-text-secondary` | grey-230 `#dbdee1` | section titles |
| `legacy-text-faded` | `#dbdee1` at 30% | `#686a70` |
| `legacy-text-muted` | primary-400 `#72767d` | — |
| `legacy-text-link` | hsl(197 100% 47.8%) `#00aff4` (2018 canonical `#00b0f4`) | — |
| `legacy-chat-bg` (composer) | `#72767d` at 30% | **`#484b52`** on chat |
| `legacy-guild-separator` | white 20% | `#4d4e51` on `#202225` |
| `status-online / idle / dnd / offline` | `#43b581` / `#faa61a` / `#f04747` / `#747f8d` | — |
| `background-feedback-notification` (mention badge) | `#f04747` | — |
| `badge-background-brand` | `#7289da` | — |

### 2.3 Mapping onto `theme.rs` (dark)

2018 separates surfaces that `theme.rs` treats as one: the server list and the channel sidebar, and the composer and the user panel. Emerald needs **new tokens** for these (marked NEW).

| theme.rs fn | 2026 now | 2018 (DTM) | Note |
|---|---|---|---|
| `base_lowest` (server list + sidebar) | `#2c2d32` | split: **server list `#202225`** (NEW `guild_list`), **sidebar `#2f3136`** | |
| `base_lower` (chat) | `#323339` | `#36393f` | |
| `base_low` (icon without image) | `#36393f`-ish | `#2f3136`; brand `#7289da` when selected | |
| `surface_high` (composer + user panel) | `#393a41` | split: **composer `#484b52`** (`#72767d` at 30%), **user panel `#292b2f`** (NEW `user_panel`) | |
| `brand` | `#5865f2` | `#7289da` | |
| `button_brand_hover` | `#4452bb` | `#687cc6` (active `#5c6eb1`) | |
| `danger` | `#f23f43` | `#f04747` | |
| `button_danger` / `_hover` | `#d22d39` / `#a9232e` | `#f04747` / `#d84040` (active `#c03939`) | |
| `online` | `#23a55a` | `#43b581` | |
| `button_positive` / `_hover` | `#008545` / `#006c37` | `#43b581` / `#3ca375` (active `#369166`) | |
| `idle` | `#f0b232` | `#faa61a` | |
| `offline` | `#80848e` | `#747f8d` | |
| (dnd) | — | `#f04747` | |
| (streaming) | — | `#593695` [not in DTM, 2018 reference] | |
| `text_default` | `#f3f3f4` | `#dcddde` [DTM leaves message text to Discord's `--text-normal`; `#dcddde` is 2018's value; DTM's `legacy-text-primary` `#f6f6f7` is only used in settings] | |
| `text_strong` | `#ffffff` | `#ffffff` | |
| `text_muted` | `#abacb2` | `#72767d` | |
| `interactive` | `#c5c6ca` | `#b9bbbe` hover-ish / `#dcddde` hover [not in DTM, 2018 reference] | |
| `channels_default` | `#999aa1` | `#8e9297` [not in DTM, 2018 reference]. Channel icons are drawn at 0.6 opacity | |
| `interactive_muted` | `#4e5058` | `#4f545c` [2018 reference] | |
| `placeholder` | `#6d6f78` | `#72767d` [2018 reference] | |
| `link` | `#76aff6` | `#00b0f4` | |
| `feedback_positive` | `#7ecb94` | `#43b581` | |
| `feedback_critical` | `#ffa09b` | `#f04747` | |
| `hover()` | `#97979f` at 12% | `#4f545c` at 30% (`#393c41` on sidebar); member hover is the same | |
| `selected()` | `#97979f` at 20% | `#4f545c` at 48% (`#3e4248`) | |
| `active()` | `#97979f` at 16% | `#4f545c` at 30% while pressed | |
| `border()` | `#97979f` at 12% | several: header shadow `#202225` at 70% 1px + black 6% at 2px; group separator white 4% (`#3e4147`); composer top rule white 6% (`#42454b`); toolbar dividers `hsla(0,0%,85%,.1)` | |
| `mention_text` / `mention_bg` | `#c9cdfb` / `#5865f2` at 30% | text `#7289da` on `#7289da` at 10%, **radius 0**; hover: bg `#7289da` (text white) | |
| `mentioned` / `_hover` / `_bar` | `#f0b232` at 10% / 16% / bar | body `#faa61a` at 10%; a strip `#faa61a` at 20% 2px wide plus a solid `#faa61a` 4px left border (§5) | |
| `jumped` | `#5865f2` at 20% | `#7289da` at 20% | |
| `text_selection` | `#5865f2` at 40% | `#7289da` at 40% (no DTM rule) | |
| `reaction_mine` | `#5865f2` at 15% | `#7289da` at 30%; count text `#7289da` | |
| (reaction normal) | — | `#dcddde` at 10% (`#47494f`); count `#72767d` | |
| `code_bg` | `#1e1f22` at 60% | code block `#2f3136` with a 2px `#292b2f` border; inline code keeps Discord's bg (2018: `#2f3136`) | |
| `spoiler_hidden` | `#1e1f22` | no DTM rule (2018: `#202225`) [reference] | |
| `scrim` | black 72% | no DTM rule (2018 modal backdrop black 85%) [reference] | |
| (timestamp) | — | white 20% (`#5e6165`); hover/alt `#99aab5` | |
| (date divider text) | — | `#99aab5` at 50% (`#68727a`), 14px 500 | |
| (unread divider) | — | line `#f04747` at 40%, label `#f04747` at 80% | |
| (scrollbar) | — | 14px; track black 10%, thumb black 40%, with a 3px border in the surface color and radius 7px | |

Code highlight (Solarized-like): base text hsl(186 8.3% 55.1%) `#839496`; comment `#586e75`; keyword/addition/selector-tag `#859900`; string/number/literal/regexp/doctag `#2aa198`; title/name/section/selector-id/class/operator `#268bd2`; attr/type/variable/template-var `#b58900`; bullet/link/meta/symbol/subst `#cb4b16`; built_in/deletion `#dc322f`; formula bg `#073642`.

### 2.4 Light theme

**DTM-18 defines no light values for any `--legacy-bg-*`, `mod-*`, `input-*`, `text-*`, `icon-*`, `chat-bg` or `status-*` token.** In light mode those `var()`s are undefined, so surfaces fall back to Discord's defaults or to transparent (§7). The only explicit light values are:

- channel header title `#4f545c`
- search bar `rgba(246,246,247,.6)`; its divider `hsla(218,5%,47%,.3)`
- search icons `#4f545c` at 0.6 opacity (1.0 on hover)
- message-group separator `#eceeef`
- timestamp `#99aab5`
- ephemeral message bg `hsla(0,0%,95.3%,.6)` fading to transparent at 80%
- reactions `rgba(227,231,248,.6)`
- composer top rule black 10%; attach divider `#e8e8e9`
- upload card dashed border black 10%; upload media border `#4f545c`; filename black 80%
- embed grid `hsla(0,0%,97.6%,.3)` with border `hsla(0,0%,80.4%,.3)`
- chat scrollbar thumb `#7289da`
- channel unread dot black 30% (white 60% on dark)

Suggested light palette, using the theme's own **grey** scale (Discord's pre-2023 light tokens) [inferred, not set by DTM]:

| Token | Value |
|---|---|
| server list | `#e3e5e8` (grey-200) |
| sidebar / member list | `#f2f3f5` (grey-130) |
| chat | `#ffffff` |
| user panel | `#ebedef` (grey-160) |
| composer | `#ebedef` |
| text | `#2e3338` (grey-700) |
| strong text | `#060607` |
| muted text | `#747f8d` (grey-500) |
| channels | `#6a7480` (grey-530) |
| hover | `#747f8d` at 16% |
| selected | `#747f8d` at 24% |
| link | `#00b0f4` |
| brand and status | same as dark |

## 3. Typography

- **Family:** `"Whitney Asset", "Whitney Data", "gg sans", "Noto Sans", "Helvetica Neue", Helvetica, Arial, sans-serif`. Display and headline use the same stack.
  - "Whitney Asset" loads `/assets/<hash>.woff` from discord.com: weights 300, 400, 500, 600 and 700, hashes `6c6374ba…`, `e8acd7d9…`, `3bdef125…`, `be0060da…`, `8e12fb4f…`. These files are probably gone now (§7).
  - "Whitney Data" is the same 5 weights embedded as base64 **WOFF 1.0**, 76–83 KB each, declared with the bogus MIME `@file/octet-stream`.
  - Whitney is a commercial Hoefler&Co font. Emerald should not bundle it. Use gg sans or Noto Sans, or load a font the user supplies.
- **Mono:** `Consolas, "Liberation Mono", Menlo, Courier, monospace`. Emerald's `MONO` already matches.
- **Weights:** 300 for offline member names; 400 body; 500 channel names, badges, embed titles, reaction counts and dividers; 600 headers and settings titles; 700 settings category labels and slider marks.

| Element | Size / line height / weight |
|---|---|
| Message body | 15px (0.9375rem) / 1.3 (19.5px) / 400 |
| Message header row (username + timestamp) | height 1.3rem (20.8px). Username size and weight are Discord's: 16px / 500 [DTM leaves it] |
| Timestamp | Discord's default 12px (0.75rem); `margin-left: .3rem`; white 20% |
| Composer text | 15px / 20px line height, letter-spacing −0.4px (−0.025rem) |
| Typing indicator | 13px / 24px line height / 400, row height 26px |
| Channel names (sidebar) | weight 500 (size unchanged, Discord's 16px) |
| Chat header channel name | 18px / 600, height 22px; `#ffffff` dark, `#4f545c` light |
| Settings page heading (h2) | 16px / 20px / 600, UPPERCASE |
| Settings section titles | 12px / 16px / 600, UPPERCASE, letter-spacing 0.5px |
| Settings sidebar category labels | 12px / 16px / 700, UPPERCASE, `#72767d` |
| Settings descriptions | 14px / 20px / 500, `#72767d` |
| Radio option titles | 16px / 24px / 500 |
| Buttons | 14px / 16px |
| Embed body / title / footer | 15px/1.3/400; 14px/500; 12px/12px/500. Provider name `#87909c` |
| Code block | 14px (0.875rem) / 16px |
| Inline code | 85% of the surrounding size |
| Reaction count | 14px / 500, letter-spacing 0.13px |
| Badges (NEW, number) | 12px / 12px / 500, with a 1px text-shadow |
| Bot tag | 10px (0.625rem) / 1.3 / 500 |
| Guild (clan) tag text | 10px, muted |
| Unread / date divider labels | 14px / 500 |
| Game-profile section titles | 14px / 600, UPPERCASE, line height 1.14, `#dbdee1` |

## 4. Shape

| Item | Value |
|---|---|
| Server icons | 50×50. **Circle (radius 25px) at rest, 15px radius when selected.** The morph is a 1.5s spring easing. DTM does not set a hover radius; 2018 used 15px on hover too [reference]. |
| Folder (closed) | 50×50, radius 15px; bg = folder color at 25% (`#343c52` for the default). Mini icons inside are round, with 5px padding; a `#4f545c` underlay fades in on hover. |
| Add-server button | 50px circle, **1px dashed border** in currentColor, color white 20% (75% on hover); "+" glyph 38.4px, weight 300. |
| Server-list separator | 30×2px, white 20%, square ends |
| Message avatars | 40px circle (Discord default), 0.8 opacity on hover |
| Avatar decoration | 46.5px around the 40px avatar |
| Buttons | radius 3px |
| Inputs / search | radius 3px |
| Composer box | radius 5px |
| Code block | radius 5px |
| Inline code | radius 3px |
| Embeds / attachments / reactions / channel rows / member rows / settings items / radio rows | radius 3px |
| Mention pill | radius 0 |
| Message action-bar buttons | no box |
| Badges (number, NEW, text, bot, live, guild tag) | radius 3px (Discord uses pills). Voice user-limit chip radius 2px. |
| Settings cards, preview boxes, nav icons | radius 5px |
| Settings close button | circle 36px |
| Switch | pill 42×24 |
| Slider track / grabber | track radius 4px; grabber radius 3px |
| Upload card | radius 10px 10px 0 0 |
| Modals / popouts | **not restyled** (Discord defaults) |

**Shadows:**
- Header bars (sidebar header, chat header, game-profile bar): `0 1px 0 rgba(32,34,37,.7), 0 2px 0 rgba(0,0,0,.06)` in place of a border.
- Badges: `0 1px 0 rgba(0,0,0,.25), inset 0 1px 0 rgba(255,255,255,.15)`, with text-shadow `0 1px 0 rgba(0,0,0,.25)`. This is the glossy 2018 badge.
- Switch knob: `0 2px 4px rgba(0,0,0,.3)`; track `inset 0 1px 1px rgba(0,0,0,.15)`.
- Slider grabber: `0 3px 1px rgba(0,0,0,.05), 0 2px 2px rgba(0,0,0,.1), 0 3px 3px rgba(0,0,0,.05)`.
- GIF result on hover: `0 11px 22px 1px rgba(4,4,5,.3)` plus an inner ring of 2px brand and 3px `#2f3136`.
- Upload media: `0 2px 8px rgba(0,0,0,.4)`, growing to `0 4px 16px` on hover.
- Message action bar: **none**.

**Borders and separators:**
- Message groups: 1px white 4% (`#eceeef` on light), inset 20px from both sides.
- Composer area: 1px top rule, white 6%.
- Attach button: right divider 1px white 10%, drawn only from 10% to 90% of the height.
- Toolbar: dividers 1px × 24px.
- Inputs: 1px black 30%; `#040405` on hover; brand on focus.
- Settings cards: 1px `#202225`.

## 5. Layout per area (2018 vs Emerald 2026)

**Server list**
- Width **70px** (now 72). Icons **50px** (now 40). The 10px gap and 10px top padding are both set by the theme.
- Icons are circles that become 15px-radius squircles when selected; now always 12px.
- Separator 30×2px (now 32×1).
- Add-server button: dashed circle.
- Mention badge sits at top −6px / right −6px with a round `#202225` bg. The lower badge (voice/stream) sits at −2/−2. The voice "connected" icon badge is 24px with 2px padding, positive color.
- Selected icon with no image: bg `#7289da`; at rest `#2f3136`.
- Unread pill: Discord default (4px wide).

**Channel sidebar**
- bg `#2f3136`. Width = Discord's `--custom-guild-sidebar-width` − 70; DTM never sets it. 2018 used **240px** [reference]; Emerald's 2026 sidebar is about 350px in the screenshots.
- Header 48px, no border, shadow as in §4.
- Channel rows: padding 0 8px, radius 3px, icon 16px at 0.6 opacity, name weight 500, gap 4px. Pressed: `#4f545c` at 30%. Selected: `#4f545c` at 48%.
- Inline actions (invite / settings) 18px at 0.3 opacity (0.6 on hover), white.
- Voice user-count chip: 16px tall, bg `#36393f`, radius 2px, mono 12px "n/limit".
- Live voice channel: 4px positive left bar at 0.6 opacity.
- The top-left corner is square (8px on Windows).

**User panel (bottom-left)**
- bg `#292b2f`, flush, **no rounding, no margin**, spans the channel column only (not the server list).
- Mic/deafen chevrons hidden. Buttons have radius 3px and gap 0.
- Height is Discord's default (2018: 52px [reference]; Emerald: 56px floating card).

**Chat header**
- 48px, bg `#36393f`, padding 0 8px 0 12px, shadow as above.
- Hash icon 16px in `#8a8e94`, margin-right 3px. Name 18px/600. The topic follows after a divider.
- Toolbar icons 24px in 32×32 hit boxes, margin 0 4px, with a 10px left margin. Separator 1px×24px.
- Search box **144px** wide, growing to **244px** when focused. Height 28px, bg white 10%, radius 3px, inner padding 3px 6px. A 1px×22px divider follows it.
- Icon badges get a 2px ring in the header bg color.

**Message rows (cozy)**
- Left inset 80px (Emerald now 72). Avatar 40px at left 20px. Right padding 10px.
- Group gap = `--f-msg-spacing`, split above and below; the default is 20px + 20px. Continuation lines have 0 vertical padding.
- Header 20.8px tall. Timestamp 0.3rem after the name.
- Body text keeps a 94px float on the right, so text never runs under the action bar.
- **Every group boundary has a 1px separator inset 20px left and right** (the signature 2018 look). Emerald has none today.
- Replies: the reply line is 20.8px tall with 0.2rem below it. The reply avatar is hidden. The spine uses interactive-muted with a 3px top-left radius and a 20px gutter.
- Mention highlight:
  - bar at left 20px: 4px solid `#faa61a` border plus a 2px strip of `#faa61a` at 20%, radius 3px 0 0 3px;
  - body from 26px to right 10px: `#faa61a` at 10%, radius 0 3px 3px 0;
  - the content shifts right by 20px (the avatar moves to 40px).
- Ephemeral messages get the same 20px shift and a left-to-right gradient `#2f3136` at 30%, transparent from 80%.
- System messages: inset 48px. 16px icons sit to the left of the text: join = green → arrow `#43b581`, leave = red ← arrow `#f04747`, call start (green phone), call end (grey `#99aab5`), edit (grey pencil).
- No hover background: the row background stays transparent. Jump flash: brand 20%.
- Unread divider: red line; "NEW MESSAGES" label centered on the chat bg, with no pill cap. Date divider: inset 20px.

**Hover action bar on messages**
- A bare row of icons, **not a floating card**. Pinned to the right edge (16px), level with the first text line. It sits below the header, offset 1.3rem on group starts and 2.8rem with a reply.
- bg = chat bg (blended 10% with warning on mentioned messages), no border, no shadow, radius 0, with a 10px gradient fade on its left edge.
- Buttons are 16px icons with 4px spacing, color `#99aab5` at 0.4 opacity (1.0 on hover). Danger buttons are red. Separators 16px tall.
- Emerald 2026 has a bordered pill with quick reactions; 2018 is about 3–5 small icons. 2018 had react / edit / "…" [reference].

**Reactions**
- Height ≈ 22px (16px emoji + 3px top/bottom) vs 28px now. Margin 2px, padding 0 6px, no border.
- Count sits 6px after the emoji. Super-reaction glow hidden.
- The add-reaction button is 16px, grey, visible only while the reaction row is hovered.

**Embeds / attachments**
- Embed: 4px colored left border, transparent body. The inner grid has a translucent bg and border with radius 0 3px 3px 0, padding 4px 10px 8px, margin-top 4px. Author icon 20px. Media margin-top 8px.
- Images: radius 3px.
- On hover a black gradient (90% to transparent, about 72px) slides down 40px from the top, carrying white 24px icons at 0.6 opacity (1.0 on hover).
- File cards: hover icons at the right, 10px in.

**Composer and its buttons**
- The form has 20px horizontal padding. Above it: a 1px top rule and 20px padding. Below it: **30px** margin, where the typing indicator lives.
- Box: bg `#484b52`, radius 5px, no border.
- **Attach button on the left**: 24px icon in 10px padding, full height, with a 1px divider on its right (10–90% of the height). Opacity 0.3, going to 1.0 and scale 1.1 on hover.
- Text: padding 10px, min 20px, max **144px** before scrolling, 4px scrollbar.
- Right buttons (gift / GIF / sticker / emoji): row 44px tall, margin-right 6px, gap 0. Each button 36×36 with 6px padding and a 24px icon at 0.3 opacity, going to 1.0 and scale 1.15 on hover.
- Emoji button: **2018 colored emoji sprite** (`--img-emojiShuffle`, PNG 960×192, 24px cells, grayscale until hovered).
- Disabled box: 0.5 opacity.
- Upload preview cards stand above the box with a 2px dashed border, radius 10px 10px 0 0, min 90px tall, and media that lifts 6px on hover.

**Typing indicator**
- **Below the composer**, in the 30px bottom gutter: absolute, bottom 1px, height 26px, 13px font. The text starts 4px after the dots, with 0 left margin.
- Emerald 2026 currently draws it above the composer.

**Member list**
- bg `#2f3136`. Width is Discord's default (2018 240px [reference]).
- Rows: radius 3px; hover `#4f545c` at 30%.
- **Offline members are not faded.** Instead the avatar is grayscale(1), the name grayscale(0.5), and the username weight 300; the color comes back on hover.
- Role headers are not restyled.

**User popout / profile modal**
- **Not restyled by DTM-18.**
- 2018 reference [unverified]: popout 250px wide, header colored `#7289da` or `#202225`, avatar 90px centered, name 16px/600, body `#2f3136`, "ROLES" uppercase 12px, a "Message @user" input at the bottom. Profile modal 600px wide with a dark header, 80px avatar, uppercase underlined tabs (User Info / Mutual Servers / Mutual Friends).

**Settings screens**
- **Full-window layer** (not Emerald's floating rounded modal). Backdrop `#202225`; content `#36393f`.
- Sidebar `#2f3136`, flex-grows from the left with items aligned to the right edge. Nav column 218px with padding 60px 6px 60px 20px.
- Items: padding 6px 10px, radius 3px, no icons. Active: bg `#7289da` with white text. Gap 2px within a section and 10px between sublists. Section separator 1px inset 10px. Category labels 12px/700 uppercase.
- Content panel: padding 60px 40px 80px, min 460 / max 820px, margin-right 81px.
- Close button: 36px circle with a 2px `#72767d` border and 18px X, at top 60px. An "ESC" label sits 8px below it (13px/600 uppercase). Breadcrumbs hidden.
- Clickable rows: cards with `#202225` at 60% bg, 1px `#202225` border, radius 5px, padding 20px. Title 12px/600 uppercase faded; value 16px/400. Trailing text is 12px uppercase with a 1px×24px divider before it.
- Nav chevron box: 48×48, radius 5px.
- Controls (see §4): radio rows show a 24px square box that fills with brand and a white check when selected. 2018 switch 42×24. Slider: 8px track and 10×24 white grabber, marks 10px/700.

**Modals (image viewer)**
- Not restyled. 2018 reference: image centered on black at 85%, an "Open original" link (14px, below the image), no top toolbar [unverified]. The screenshot shows the 2026 modal with a toolbar.

**Emoji / sticker / GIF pickers**
- Only these are styled: the GIF result hover ring and shadow, and reaction-list rows (radius 2px, hover `#4f545c` at 10%).
- The rest is Discord's default. The 2018 picker [reference] had no stickers or GIF tabs, a search on top, and a category column; Emerald should keep its tabs and recolor them with §2.

**Voice panel**
- Only the panel bg (`#292b2f`, it shares the panels container) and 3px-radius buttons.
- 2018 reference: a "Voice Connected" strip above the user panel, green `#43b581` 14px/600 text, with channel / server in 12px muted [unverified].

**DMs / home / friends**
- No specific rules. Emerald gets the 2018 look from tokens: sidebar `#2f3136`, rows radius 3px, selected `#4f545c` at 48%.
- 2018 friends page reference: tabs "Online / All / Pending / Blocked" plus a green "Add Friend" button in the 48px header [unverified].

**Discover / game profile**
- Discovery did not exist in 2018. DTM restyles the 2026 game-profile page:
  - 48px compact bar with the header shadow;
  - hero art 154px tall, max 880px, with an ellipse mask;
  - content max 1200px with padding 20px;
  - cards: `#202225` at 48% bg, 1px `#202225` border, radius 5px, `#202225` at 60% on hover;
  - section titles 14px/600 uppercase;
  - detail rows `#2f3136`, radius 3px, gap 2px;
  - sidebar margin 40px.
- The same card treatment works for Emerald's discover grid.

## 6. 2026-only features

| Feature | DTM-18 treatment |
|---|---|
| Avatar decorations | **Kept**, resized to 46.5px around the 40px message avatar. Not hidden. |
| Nameplates (member list / DM rows) | **Untouched.** One rule hides `.background__4f569`, a class in the role-color module; it may be a username or role-gradient background, but this is unconfirmed. |
| Profile effects / Nitro profile themes / popout | Untouched (Discord defaults). |
| Role icons | Kept, sized 16×16 (1rem) and aligned to the top. The new-member badge is the same size. |
| Clan / guild tags | **Restyled** into a flat chip: bg `#4f545c` at 30%, radius 3px, **badge icon hidden**, text 10px muted. |
| Verified-bot badge | Its icon is replaced by a "✓" text glyph inside the brand-colored "BOT" tag (radius 3px). |
| Super / burst reactions | Glow hidden; they look like normal reactions, with a faint white inset ring when tinted. |
| App launcher / slash-command icons | Hidden; the command name shows a "/" prefix and the reply avatar is hidden. |
| Chat gradient overlay, voice-channel glow | Hidden. |
| Number / "NEW" / LIVE badges | Restyled to 3px-radius glossy rectangles; the NEW badge is warning orange. |

## 7. Broken, unknown, caveats

1. **Light theme is incomplete.** The `--legacy-bg-*`, `mod-*`, `input-*`, `text-*`, `icon-*`, `chat-bg` and `status-*` tokens only exist under `.theme-dark`. In light mode every surface that uses them gets an invalid value (transparent or Discord's default). The light values in §2.4 are a proposal.
2. `--colorblind-status` and all `--colorblind-*` SVGs are defined and never used. `--systemMsg-icon-bookmark` (pin) is also unused.
3. `--legacy-brand-160-hsl` is missing the `*` inside `calc()`, so the value is invalid (unused).
4. `--legacy-guild-iconBG: hsl(var(--legacy-primary-630))` wraps a color in `hsl()`, so it is invalid (unused).
5. `text-align: baseline` is invalid; `;;` is harmless; `.folderPreviewWrapper__48112` is listed twice.
6. **Fonts:** the "Whitney Asset" URLs point to old discord.com `/assets/*.woff`, most likely 404 now, so "Whitney Data" (embedded WOFF1, weights 300 to 700) carries the look. Its MIME type `@file/octet-stream` is wrong; browsers sniff the bytes anyway. **Whitney is licensed. Do not ship it in Emerald.**
7. The channel and member list widths depend on Discord's `--custom-guild-sidebar-width` and defaults; DTM never sets them. The 240px figures are a 2018 reference.
8. Selectors are tied to hashed class names from one Discord build. Areas whose classes changed silently fall back to the 2026 look. That explains the missing popout, picker, voice and DM styling, which was probably never written.
9. Username size and weight, timestamp size, category headers and channel-name color are left to Discord. The values given for them are 2018 references, not from DTM.
10. darker/midnight get no palette of their own; they use `.theme-dark`'s.
11. The "NEW MESSAGES" rewrite only applies to en-US (Emerald would localize it anyway).

**Embedded assets worth extracting (in `dtm18.css`):**
- Five WOFF1 fonts (do not ship).
- `--img-emojiShuffle`: PNG 960×192 sprite for the composer emoji button, 24px cells, about 85 KB.
- Small inline SVGs (`image/svg+xml`, URL-encoded, not base64):
  - system-message icons, 18×18: join `#43b581` arrow, leave `#f04747` arrow, call start `#43b581`, call end `#99aab5`, edit `#99aab5` pencil, pin;
  - colorblind status glyphs, 10×10;
  - switch knob X / check, 18×18;
  - the "sticker/lottie" button mask, 18×18.
- All the SVGs are trivial to redraw as Emerald icons.
