# App shell / navigation: Discord 2026 screenshots vs Emerald code

Paths are relative to `D:\app\emerald\emerald-ui\src` unless stated.
Confidence: **[code]** = certain from reading the code; **[inferred]** = measured/guessed from screenshot pixels (screenshots are light theme, ~1869x983 at 100% scale; server icons measure 40px, so scale is 1:1).

Note: all 5 screenshots are the **light theme**. Light token values in theme.rs (base_lowest #f3f3f4, base_lower #fbfbfb, etc.) look consistent with the screenshots; no color-token mismatch was obvious. The main gaps are structural (missing elements), not palette.

---

## 0. Global frame (all screenshots)

| # | Screenshot | Code | Conf. |
|---|---|---|---|
| 0.1 | Custom **title bar** (~28-30px) across the whole window: back/forward arrows at left, centered page icon + title ("Amigos", "illusia", "Descubra"), right side: Inbox icon, Help "?", green Download/update arrow, then window controls. | No custom title bar; native OS title bar with title "Emerald" (`emerald-bin/src/main.rs:120-123`, `TitlebarOptions` default). Inbox lives in the chat header instead (`app/panels/mod.rs:163-169`). No back/forward history, no help button. | [code] |
| 0.2 | Below the title bar: server list on app bg, then one panel with rounded top-left corner holding sidebar + chat. | Same idea exists: `app/render.rs:69-81` (`rounded_tl(8)`, `border_l_1`, `border_t_1`). OK. | [code] |
| 0.3 | Channel sidebar ~348px wide (x≈72→420); user panel ~402px (x 8→410). Discord 2026 sidebar is user-resizable. | Fixed 240px (`app/sidebar/mod.rs:54`), user panel `72+240-16 = 296px` (`app/user_panel.rs:38`). No drag-to-resize. | [code] width; [inferred] that screenshot width is a user resize, not default |
| 0.4 | Composer bottom edge aligns with the user panel bottom (both ~8px above window bottom, both ~56px tall); typing text sits *above* the composer. | Composer is followed by a 24px typing strip (`app/chat/mod.rs:271-280`), so the input sits ~24px above the bottom and is misaligned with the user panel (`bottom_2` = 8px, `app/user_panel.rs:37`). | [code] + [inferred] |

## 1. Server list (`app/sidebar/servers.rs`)

| # | Screenshot | Code | Conf. |
|---|---|---|---|
| 1.1 | Icons 40px, radius ~12px, pitch 48px (gap 8), white pill on the left for selected/unread. | Matches: 40px (`:243`), `rounded(12)` (`:247`), `gap_2` (`:34`), pill 4px wide (`:217-227`). OK. | [code] |
| 1.2 | Top "Home/DMs" button: Discord logo on brand (#5865f2) when selected. | Emerald logo icon (`:259-266`) on brand when selected - fine by design (own brand). | [code] |
| 1.3 | Small **speaker/voice badge** in the top-right of some server icons (voice activity in that server). | Not drawn; only mention badge bottom-right (`:280-291`). | [code] |
| 1.4 | Bottom: "+" Add server = neutral grey rounded square with a dark *outlined plus-in-circle glyph* (not green). | Text "+" 24px in `theme::online()` green, hover turns green bg (`:42-57`). That is the 2018-2023 style. | [code] |
| 1.5 | Below "+": **Discover** button (compass glyph), grey normally, brand bg + white glyph when selected. | **Does not exist.** No discover route anywhere (`grep -ri discover` over all crates: zero hits). | [code] |
| 1.6 | Folder: light tinted rounded square with blue folder glyph when open; selected server inside shows pill. | Implemented (`:84-202`, open bg `with_alpha(color,0.15)`, radius 16). Looks consistent. | [code] |
| 1.7 | Mention badge: red pill ringed with background color. | `widgets::badge` with `border_4` ring (`:167-178`, `:280-291`). OK. | [code] |

## 2. Channel sidebar - guild (`app/sidebar/mod.rs`)

Screenshots: `uma_servidor...png`, `veja_onde_fica_digitando...png`.

| # | Screenshot | Code | Conf. |
|---|---|---|---|
| 2.1 | **Server banner header**: ~128px banner image filling the top, server name (with boost/home badge glyph + chevron) overlaid bottom-left... actually top-left, white bold; "invite people" (person+) icon top-right. | Plain 48px header with name + chevron and a bottom border (`:28-46`). No banner, no invite icon. | [code] |
| 2.2 | **Boost goal bar** under the banner: rounded pill "Objetivo de impulso" with pink progress fill, "14/33 Impulsos >" at right. | Missing. | [code] |
| 2.3 | Top nav rows with icons: "Conferir Canais"/"Guia do servidor", "3 Eventos", "Canais & Cargos", then a 1px divider. | Missing (rows start directly with categories). | [code] |
| 2.4 | Category: uppercase, 12-13px, chevron after the name (`⌄`), medium weight, muted grey. | Uppercased in `app/state/spaces/rows.rs:199`, 12px SEMIBOLD, chevron after (`:136-165`). Close; weight may be slightly heavier than Discord's. | [code]/[inferred weight] |
| 2.5 | Channel rows 34px pitch, radius ~8, "#" 20px, selected bg grey; selected/hovered row shows a **person+ "create invite" button** at the right. | 34px rows (`:87`), `rounded_lg` (`:213`), icon 20px (`:258`), selected bg `theme::selected()`. **No invite/settings hover buttons** on the row. | [code] |
| 2.6 | Unread pill hugging the left edge of the sidebar (x≈74) for unread channels. | Implemented (`:221-233`). OK. | [code] |
| 2.7 | Sidebar background same color as server list (#f3f3f4 light). | Sidebar has no own bg -> inherits root `base_lowest` (`app/render.rs:63`). OK. | [code] |

## 3. DM sidebar (Home) (`app/sidebar/mod.rs`, `app/sidebar/dms.rs`)

Screenshots: `home.png`, `home_na_area_adicionar_amigo.png`.

| # | Screenshot | Code | Conf. |
|---|---|---|---|
| 3.1 | Top: **button** "Encontre ou comece uma conversa" - centered text, ~32px tall, radius ~8, filled slightly darker grey with a subtle border; clicking opens quick switcher. | A live `TextInput` (`dm_search`, `app.rs:308`) placed in the 48px header (`app/sidebar/mod.rs:35-37`) that filters the list in place. Visual is a text field, not a centered button. | [code] |
| 3.2 | Nav rows: **Amigos**, **Nitro**, **Loja** (Shop), **Missões** (Quests), each with 24px glyph, ~40px tall, radius 8; selected = grey bg. Then a divider. | Only "Friends" (+ "Message Requests" when any) (`:100-134`, `dms.rs:9-25`). No Nitro/Shop/Quests rows (can be links-out or omitted, but the visual block of 4 rows is missing). | [code] |
| 3.3 | "Mensagens diretas" header: sentence case, ~14px, medium, with "+" at right. | Rendered through `Row::Category` at 12px SEMIBOLD (`:145-146`); title from locale `pt-BR.toml:39` ("Mensagens diretas", not uppercased - good). Font size/weight smaller/heavier than 2026. | [code] + [inferred size] |
| 3.4 | DM rows: 32px avatar with status, name 16px, then **guild tag chip** (e.g. `ZLDA`, `KURU`, `STAR` with a small badge glyph, grey rounded chip ~4px radius) right after the name; activity/custom-status line 12px below with a game glyph. Row pitch ~44px. | Rows 42px (`:87`), name + activity line (`:276-293`). **No guild tag (primary_guild / clan) chips anywhere in the codebase** (`grep primary_guild\|clan` = 0). No activity glyph (game controller/emoji) before the subtitle. | [code] |
| 3.5 | Group DM rows: group icon, name, "5 membros" below. | Implemented (`:262-275`). OK. | [code] |

## 4. User panel (bottom-left) (`app/user_panel.rs`)

| # | Screenshot | Code | Conf. |
|---|---|---|---|
| 4.1 | Floating rounded card (~8px radius) spanning server list + sidebar, 8px inset, ~56px tall, white (light) bg with subtle border. | Matches structurally: `absolute left_2 bottom_2`, `h 56`, `rounded_lg`, `border_1` (`:34-47`). Width differs (see 0.3). | [code] |
| 4.2 | Background looks like the composer's surface (white in light). | Uses `theme::base_low()` (`:47`), while `theme.rs:44` documents `surface_high()` as "message box and the user panel surface". Code contradicts its own token doc. | [code] + [inferred visual] |
| 4.3 | Mic and headset are **split buttons**: icon + small `⌄` chevron each (chevron opens input/output device menu). | Only two 32px icon toggles, no chevrons (`app/voice/panel.rs:117-136`, `toggle` `:140-156`). | [code] |
| 4.4 | Name 14-15px semibold, subtitle 12px (custom status / "人間嫌い"); avatar 32px with status. | Matches (`:61-88`). OK. | [code] |
| 4.5 | Hover on avatar+name: rounded hover bg. | `rounded_md` hover (`:58-60`). OK. | [code] |

## 5. Friends / Home main area (`app/friends.rs`, `app/friends/list.rs`)

| # | Screenshot | Code | Conf. |
|---|---|---|---|
| 5.1 | Header: friends glyph + "Amigos", then a **small dot "•"** separator, then tabs. | Text "Amigos" with no glyph, then a 1px x 24px vertical bar (`friends.rs:146-152`). | [code] |
| 5.2 | Tabs: **Disponível, Todos** (+ Pendente only when requests exist). No "Bloqueados" tab in the bar. Selected tab: grey bg, radius ~8, 16px medium, padding ~12x4. | Tabs Online, All, Pending (always), **Blocked** (`friends.rs:153-159`); `rounded_md` (6px), `py 2`. Extra Blocked tab; Pending always shown. | [code] + [inferred: 2026 hides Pending when 0] |
| 5.3 | "Adicionar amigo" button = **brand blue (#5865f2) filled, white text**; when active = light brand tint bg with brand-colored text. | **Green** (`button_positive` #008545) filled; when active turns green text (`friends.rs:160-180`). This is the pre-2024 look. | [code] |
| 5.4 | Top-right of header: "New Group DM" icon (chat bubble +). | Missing. | [code] |
| 5.5 | **Search field** "Buscar" with magnifier at the top of the list: full width, ~40px tall, radius ~8, 1px border, white bg. | Missing - list starts with the count label (`list.rs:44-55`). | [code] |
| 5.6 | Count label "Online — 24": sentence case, ~14px medium, muted. | `widgets::label` = 12px BOLD (`widgets.rs:230-236`) with locale "DISPONÍVEL" / "TODOS OS AMIGOS" uppercase (`emerald-core/locales/pt-BR.toml:45,59`). | [code] |
| 5.7 | Friend row: avatar 32 + status, **display name + guild tag chip**, activity line with glyph (controller, voice speaker, custom emoji). Username is **not** shown beside the name (only on hover). Right side: two **round 36px icon buttons** (Message bubble, `⋮` More) on grey circle bg. Row ~62px with top border. | Row 62px with top border OK (`list.rs:133-145`). Shows username next to name always (`:173-179`). Actions are **text buttons** "Mensagem" / "Remover" (`:69-131`) - big visual difference. No tag chips, no activity glyphs. | [code] |
| 5.8 | **"Ativo agora" (Active Now) column** on the right, ~400px wide: title 20px bold, then rounded cards (radius ~8, border, light bg) per friend showing voice channel / game, with sub-row and small avatars, "+4". | **Missing entirely** (`friends.rs:189-199` is a single column). | [code] |
| 5.9 | Friends area bg: white (`base_lower`). | `base_lower` (`friends.rs:195`). OK. | [code] |

## 6. Add Friend (`app/friends.rs:202-247`)

Screenshot `home_na_area_adicionar_amigo.png`.

| # | Screenshot | Code | Conf. |
|---|---|---|---|
| 6.1 | Title "Adicionar amigo" ~20px semibold; hint 14-16px. Wumpus illustration top-right. | Title 20px semibold, hint 14px muted (`:214-225`). No illustration. | [code] |
| 6.2 | **One bordered, rounded (~12px) box** containing: username input ("Insira um nome de usuário") with the brand button "Enviar pedido de amizade" *inside* the box at right; a divider; then a second multi-line field "Personalize sua solicitação (opcional)" with a **char counter "120"** bottom-right. | Separate `TextInput` + separate brand button side by side (`:226-245`). No optional message field, no counter. Placeholder "Nome de usuário" (`pt-BR.toml:71`) vs "Insira um nome de usuário". | [code] |
| 6.3 | Caption under box: "O que você escrever aqui também aparecerá nas suas mensagens diretas...". | Missing. | [code] |
| 6.4 | Section "Outros lugares para fazer amigos" + text + card button "Explorar Servidores Públicos" (green compass tile, chevron right, bordered card radius ~8). | Missing (depends on Discover). | [code] |
| 6.5 | Active Now column stays visible on the Add tab. | N/A (missing). | [code] |

## 7. Discover (`área_de_discover.png`)

**Does not exist** in Emerald. No `discover` string in any crate (Rust or locale). [code]

Screenshot layout, for reference:
- Server-list bottom compass button selected (brand bg).
- Left sidebar: title "Descubra" (20px bold) in place of the guild header; rows **Apps**, **Servidores** (selected, grey bg radius 8), **Missões**, each with 24px glyph, ~48px tall.
- Main: top tab bar with a glyph, tabs "Início" (selected, underlined 2px), Gamer, Música, Entretenimento, Ciência e Tecnologia, Educação, Centrais de Estudantes; right "Buscar" field (radius 8, border).
- Hero: lavender gradient band ~300px tall, huge heavy uppercase headline (ABC Ginto Nord-like display font) "ENCONTRE SUA COMUNIDADE NO DISCORD", subtitle.
- "Servidores em Destaque": 4-column grid of cards (radius ~8, 1px border): banner image ~146px, 48px server icon with rounded-square (~14px radius) white ring overlapping banner bottom, verified/partner green check + name bold, description (3 lines), footer "● 1.083.530 online  ● 4.611.889 membros" 12px.
- Implementing it needs `GET /discoverable-guilds` / discovery endpoints (not in emerald-discord today) - read-only, but must be imitated from the web bundle per CLAUDE.md.

## 8. Chat header (`app/chat/mod.rs:91-196`, `app/panels/mod.rs:113-170`)

| # | Screenshot | Code | Conf. |
|---|---|---|---|
| 8.1 | "#" glyph 24px, then channel emoji/name 16px semibold, then **"•" dot separator** and topic (14px muted). | Uses a 1px x 24px vertical bar before topic (`chat/mod.rs:142`). | [code] |
| 8.2 | Right tools order: Threads, **Notification bell** (with slash when muted), Pins, Member list toggle, then **search box**. | Order: thread join button, call/add-DM, member toggle (`:163-195`), then threads, pins, search, **Inbox** (`panels/mod.rs:118-169`). No bell/notification-settings button in header; Inbox should move to the title bar. Member toggle appears *before* threads/pins (2026 has it after pins). | [code] |
| 8.3 | Search box: "Buscar <server name>" placeholder, ~240x32, radius 8, **1px border on white** (light), magnifier icon on the **right**. Its left edge aligns with the member list column. | 220x28, `rounded_md`, filled `base_lowest`, magnifier on the **left** (`panels/mod.rs:150-160`); placeholder generic "Buscar" (`pt-BR.toml:480`). | [code] |
| 8.4 | Header ~48px, border-bottom. | 48px + `border_b_1` (`chat/mod.rs:92-99`). OK. | [code] |

## 9. Member list (`app/members.rs`, `app/members/row.rs`)

Screenshots: `uma_servidor...png` (title remark: "everything is a bit rounded; roles carry **badges/insignias, not emojis**"), `veja_onde_fica...png`.

| # | Screenshot | Code | Conf. |
|---|---|---|---|
| 9.1 | Group headers are **sentence case as the role is named** ("Staff — 7", "Líder — 1", "Classe S — 4"), 12-13px medium, **preceded by the role's icon** (shield, heart, gem, etc. 16px image). | Forced `to_uppercase()` (`members/row.rs:187`), 12px SEMIBOLD, **no role icon** (`:28-43`). Role `icon` / `unicode_emoji` are not even in the model (`grep` over emerald-discord: none). | [code] |
| 9.2 | Member rows: name in role color, then **guild tag chip** (e.g. `ILUS`, `KoC`, `HOLY`, `NM` — grey rounded ~4px chip with a small badge image + 4-letter tag, 12px bold), plus small icons (boost gem, etc.). This is the "insignias" the user means. | Only an `APP` tag for bots (`:153`, `app_tag :192-202`). No guild tag chips, no role icons, no booster icon. | [code] |
| 9.3 | Rows ~44px, radius ~8, nameplate gradient + image on right (Nitro nameplates), offline faded. | Matches: 44px (`members.rs:35`), `rounded_lg`, nameplates (`row.rs:104-122`), opacity 0.3 offline (`:133`). OK. | [code] |
| 9.4 | **No search box at top**; instead (when present) an **"Atividade — 1 ⚙" section** with a rich activity card (album art, "Spectrum", "18 faixas esta semana") before the role groups. | Search input pinned at top (`members.rs:160`). No activity section. | [code] |
| 9.5 | Width ~256px (x≈1612→1869); background looks same as chat (white in light), separated by a border. | 240px (`members.rs:34`), bg `base_lowest` (sidebar grey) (`:159`). | [code] + [inferred bg & width] |
| 9.6 | Some names render with **gradient/"enhanced role" colors** and custom display-name fonts (e.g. "Glauber1907", "Azriel Arroz" in a different font). | Single solid role color (`row.rs:72-77`). No display-name styles. | [code] + [inferred gradient] |
| 9.7 | Role icon also appears beside the author name **in chat messages** (e.g. "Scizor Tohsaka" + blue person icon, "Aokari" + red emblem, "Glauber1907 ⚔KoC 🔥icon"). | No role icons in message headers either (role icon data not modeled). | [code] |

## 10. Typing indicator (`app/chat/typing.rs`, `app/chat/mod.rs:271-280`)

Remark in filename: "see where typing is shown now".

| # | Screenshot | Code | Conf. |
|---|---|---|---|
| 10.1 | Typing line sits **above the composer**, overlaid at the bottom of the message list (y≈907 vs composer 920-975), left-aligned with the avatar column (~x 446). | Rendered in a 24px strip **below** the input (`chat/mod.rs:271-280`). | [code] |
| 10.2 | Starts with an **animated three-dot "•••" glyph** (~24px wide), then "**Scizor Tohsaka** está digitando..." 12px, name bold. | Text only, no animated dots (`typing.rs:9-38`). Wording/bold OK (`pt-BR.toml:177`). | [code] |
| 10.3 | Composer: rounded (~8px) box, 56px, white surface with 1px border; left "+" in circle; right buttons Gift, GIF, Sticker, Emoji, Apps grid. Placeholder "Conversar em # 🎃 ‖ bate-papo". | Only "+" (when polls/files allowed) and emoji button (`chat/mod.rs:262-268`). (Composer details likely covered by another comparison.) | [code] |

## 11. "Everything a bit rounded" remark

Radii in code are mostly right already: server icons 12px, rows `rounded_lg` (8px), user panel 8px, panel corner 8px. Spots still square-ish / 2018-ish:
- Friends tabs and add button `rounded_md` (6px) and `py 2` -> 2026 tabs are 8px with more padding (`friends.rs:119,166`).
- Header tool buttons & svg buttons `rounded_md` (`widgets.rs:220`, `chat/mod.rs:387`).
- Friend actions are rectangular text buttons instead of circles (`friends/list.rs:69`).
- Search fields: 2026 uses 8px radius with 1px border; code uses `rounded_md` filled (`panels/mod.rs:157`).
- Missing rounded cards (Active Now, Discover cards, Add Friend combined box radius ~12).
- App tag radius 3px (`members/row.rs:196`) vs ~4px chips in 2026.

## Ranked by visual impact

1. Missing custom title bar (back/forward, centered title, inbox/help/download) [0.1].
2. Guild header: no banner, boost goal bar, Server Guide/Events/Channels & Roles rows [2.1-2.3].
3. Home: missing "Active Now" column, search field, round icon actions; green Add Friend (should be brand blue); extra Blocked tab; uppercase count label [5.x].
4. No guild tag chips anywhere (DM list, friends, member list, chat) and no role icons (member-list group headers, beside names) — the user's "insignias, not emojis" remark [9.1, 9.2, 9.7, 3.4].
5. Typing indicator below the composer instead of above it with animated dots; composer not bottom-aligned with user panel [10.1-10.2, 0.4].
6. Discover does not exist (no compass button, no screen) [1.5, 7].
7. DM sidebar: text field instead of "Find or start a conversation" button; no Nitro/Shop/Quests rows [3.1-3.2].
8. Chat header: dot vs bar separator, bell button missing, search box style/position, inbox in wrong place [8.x].
9. Add Friend form: single combined box with button inside + optional message + counter + "Explore public servers" card [6.x].
10. User panel: no chevrons on mic/headset, bg token mismatch, width [4.x]; add-server "+" green instead of neutral glyph [1.4]; voice badge on server icons [1.3].
11. Member list: search box at top instead of activity section; uppercase group names; width/bg [9.4-9.5, 9.1].
