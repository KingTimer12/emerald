# Settings, profile editor, mic/headset menus: Discord 2026 vs Emerald

Screenshots are the LIGHT theme at ~100% scale (~1870x977). Pixel colors sampled with System.Drawing.
Confidence: H = measured/obvious, M = estimated from the picture, L = guess.

## 1. User settings (config_do_usuario.png) vs emerald-ui/src/app/settings.rs

### Container
| # | Discord 2026 | Emerald | Conf |
|---|---|---|---|
| 1 | **Centered modal**, not full screen: ~1400x845 (x 234-1634, y 66-910), ~12px radius, 1px border #dfdfe1; app behind dimmed by scrim (#f3f3f4 -> #787878, black ~50%, matches `theme::scrim()` light 0x85). | Full-window opaque overlay `.size_full().bg(base_lower)`, no scrim, settings.rs:248-256. | H |
| 2 | Close = plain **X icon** (~16px, #28282d) at the **top-right of a header bar**; no "ESC" caption, no ring. | 36px bordered circle with a "✕" glyph + "ESC" label in a 72px right column, settings.rs:216-246. | H |
| 3 | Content pane has a **header bar** (~50px tall, page name "Conta" 15px regular at left, 1px bottom border #dfdfe1). | No header bar; 20px semibold title inside the scroll area, settings.rs:274-281. | H |
| 4 | Sidebar and content share one bg (#fbfbfb = `base_lower` light), split by a 1px vertical border #dfdfe1. | Sidebar is `base_lowest` (#f3f3f4), settings.rs:172; no border. | H |
| 5 | Content column centered, max-width ~700px (x 712-1409), top padding ~70px. | Left-aligned, `max_w(740)`, `px_10`, `pt(60)`, settings.rs:259-268. | M |
| 6 | Thin overlay scrollbar, thumb #8f9097 (~4px). | gpui default. | M |
| 7 | Divider color #dfdfe1 is darker than `theme::border()` (12% overlay ~= #efefef on #fbfbfb); looks like `--border-normal` (~24%). | Only `border()` exists, theme.rs:166-168. | M |

### Sidebar
| # | Discord 2026 | Emerald | Conf |
|---|---|---|---|
| 8 | **User header**: avatar ~48px, display name 15px, "Editar perfis" + pencil (opens the profile editor). | None; starts with a "User Settings" caption, settings.rs:173. | H |
| 9 | **Search box** ("Buscar", magnifier, ~40px tall, radius 8, bg #f6f6f6, 1px border). | None. | H |
| 10 | **Icon before every item** (~20px: person, shield, chat-lock, bell, Nitro, boost, subscriptions, gift, card, mic, palette...). | Text-only items, settings.rs:142-158. | H |
| 11 | Items ~36px tall, 15-16px **regular** weight, dark text for selected and unselected, radius 8, selected bg #e7e7e9 (= `theme::selected()` on #fbfbfb, matches). | `py(6)` (~32px), `FontWeight::MEDIUM`, unselected in `interactive()`, `rounded_md`, settings.rs:146-152. | M |
| 12 | **Sub-navigation** under the selected page: indented anchors (Informações da conta / Senha e segurança / Status da conta / Central da Família) with a 1px vertical guide line, active one a dark 2px bar. Clicking scrolls the page. | None. | H |
| 13 | Section captions in **sentence case**, ~13px muted, not bold ("Cobrança", "Experiência"); the first group has no caption. | `widgets::label` 12px BOLD muted (widgets.rs:230-236), "User Settings"/"App Settings". | H |
| 14 | Groups: [Conta, Dados e privacidade (+ "NOVO" pill), Permissões de mensagens, Notificações] / Cobrança [Nitro, Impulso de servidor, Assinaturas, Inventário de presentes, Cobrança] / Experiência [Voz e vídeo, Aparência, ...]. Profiles is NOT a sidebar page (it's the "Editar perfis" link); devices are a row inside Conta; online/idle/dnd status is not a settings page. | [My Account, Profiles, Privacy, Devices, Accounts, Status] / [Appearance, Voice, Shortcuts, Language, Desktop] / Log out, settings.rs:174-200. | H |
| 15 | Separators span the sidebar width (x 251-471), #dfdfe1. | `mx_2p5 my_2` with `border()`, settings.rs:159. | M |
| 16 | "NOVO" badge: brand #5865f2 pill, white bold ~10px. | No badge widget. | M |

### Account page (settings/account.rs)
| # | Discord 2026 | Emerald | Conf |
|---|---|---|---|
| 17 | **No profile card.** A list of sections ("Informações da conta", "Senha e segurança", "Status da conta"), ~22px medium headings, 1px dividers #dfdfe1 between sections, ~55px vertical gap. | 2018-style card: 100px brand banner, 80px overlapping avatar, name, inner `base_low` box with stacked captions, account.rs:43-115. **The most "2018" element on these screens.** | H |
| 18 | Rows: label left (15px), value right-aligned ("wolfsbany", masked e-mail) + indigo "Mostrar" link + **"Editar" button** (~100x40, radius 8, bg #efefef = `hover()`, **1px border #dddddf**, 15px medium dark text). Row pitch ~72px. | Stacked caption/value; "Show" link in `theme::link()` (#006dd4 light, blue rather than indigo), account.rs:29-36, 99-104. | H |
| 19 | Rows for Telefone, Senha "Editar", "Autenticação Multifatorial — Definir >", "Dispositivos conectados — 6 dispositivos >" (chevron rows that navigate). | No phone/password/MFA rows; devices is its own page (settings/devices.rs). | H |
| 20 | Display name is not edited here (it's in the profile editor). | Inline display-name input + Save, account.rs:116-150. | H |
| 21 | Secondary button has a 1px border. | `ButtonKind::Secondary` = `hover()` bg, no border, h 38, `rounded_lg`, widgets.rs:153-176. Bg matches (#efefef); border missing; height 38 vs ~40. | M |

## 2. Profile editor (editar_perfil.png) vs settings/profiles.rs

In 2026 it's opened from "Editar perfis" and is **its own large modal on top of the settings modal** (x 340-1540, y 92-890, radius ~12, white #ffffff; close = X in a 32px bordered square button, radius 8, top-right). Three columns:

| # | Discord 2026 | Emerald | Conf |
|---|---|---|---|
| 22 | **Left editor column** (~240px, bg #fbfbfb, right border): header "Perfil principal ⌄" dropdown (main vs per-server) + "»" collapse; sections with 14px medium titles: "Placa de identificação" (guild tag row with + button), "Avatar e decorações" (two ~100x100 **tiles**: current avatar; decoration slot with +), "Cor da faixa" (one color field ~208x40), "Efeitos de perfil e molduras" (two tiles), Nitro upsell (lavender gradient #ddcbee -> #dfe1fc, lock icon, text, full-width brand "Prévia do Nitro" button), "Estilo do nome exibido" (styled-name preview), more below. | One settings page: tabs "User profile / Server profile" (profiles.rs:326-353), text inputs for name/pronouns/bio, Change/Remove avatar buttons, 11 color swatches, Save/Reset (profiles.rs:258-324). | H |
| 23 | Main/server switch is a **dropdown** in the column header. | Underlined tabs + a separate server dropdown with a "▾" glyph, profiles.rs:220-230, 398-452. | H |
| 24 | Avatar/banner changed via **image tiles**. | Text buttons, profiles.rs:239-255. | H |
| 25 | Banner color = one color field that opens a picker. | 11 swatches of 32px, profiles.rs:359-396. | M |
| 26 | **Center: full-size profile preview** (~400px card, x 628-1030, radius ~16, 1px border): banner, ~100px avatar with status badge, custom-status thought bubble, 24px bold name, "username • pronouns" + "Tag do servidor ⌄" chip, badge row, "Mensagem" (brand, disabled tint #ced2fb) + icon buttons, then sections Bio / Membro desde / Conexões (+ Adicionar conexão) / Nota. | 300px small card: 105px banner, 80px avatar, 20px name, username, pronouns, bio; profiles.rs:492-529. No status, badges, member-since, buttons or labelled sections. | H |
| 27 | **Right column: tabs "Mural / Atividade / Lista de desejos"** (underline on active, 14px), "Seus Widgets" + "Adicionar widget" secondary button, widget cards (Jogo favorito, game stats, Jogos que eu gosto), radius ~12, 1px border. | Absent (profile widgets are a 2025-26 feature; maybe out of scope). | H |
| 28 | 2026 has avatar decoration, profile effect, nameplate/frames, display-name style, guild tag, widgets; Emerald has none of these. Name/pronouns/bio inputs are probably further down the left column (not visible in the shot). | — | M |
| 29 | 2026 likely saves through a floating "unsaved changes" bar (not in the shot). | Explicit Save/Reset buttons. | L |

## 3. Microphone menu (opções_de_input_(microphone).png)

Code: `render_voice_buttons`, emerald-ui/src/app/voice/panel.rs:117-155; user_panel.rs:94-100.

| # | Discord 2026 | Emerald | Conf |
|---|---|---|---|
| 30 | Mic and headset are **split buttons**: 32px icon + a narrow (~16px) **chevron ⌄ button**. The chevron flips to ⌃ while open; pressed bg #dadadc on the chevron, #ededed on the icon. | One 32px toggle per control, no chevron, no menu, panel.rs:121-135. | H |
| 31 | The chevron opens a **popover anchored above the button**, ~220px wide (x 256-476), bg #ffffff (= `surface_high` light), 1px border #e2e2e4, radius ~8, padding ~8, shadow. | Missing. The user menu container (user_menu.rs:153-163: 220px, p_2, rounded_lg, border, surface_high, shadow_lg) matches and can be reused. | H |
| 32 | **Two-line rows**: "Dispositivo de entrada" (14px) / "Realtek(R) Audio" (12px muted) with `>` opening a device submenu; "Perfil de entrada" / "Personalizado" with `>` (voice isolation / studio / custom). | `place_menu::item` is one-line 32px only (place_menu.rs:269-285). Devices are chips on the Voice page (settings/voice.rs:36-54). No input-profile concept. | H |
| 33 | 1px separators #e2e2e4. | Menu separators exist (user_menu.rs:58). | H |
| 34 | "Volume de entrada" (14px) + **continuous slider**: ~4px track, brand #5865f2 fill, gray #c5c6ca rest, white round ~16px knob with light border/shadow. | No slider widget in the codebase (grep "slider" finds nothing). Volume is 6 fixed chips 50-200% on the Voice page, settings/voice.rs:8, 56-62. | H |
| 35 | Footer row: gear icon + "Configurações de voz" (opens Voz e vídeo). | — | H |

## 4. Headset/output menu (opções_de_output_(headset).png)

| # | Discord 2026 | Emerald | Conf |
|---|---|---|---|
| 36 | Same popover (x 312-530): "Dispositivo de saída / Realtek(R) Audio >", separator, "Volume de saída" slider (~50% here), separator, gear "Configurações de voz". No profile row. | Missing (as above). | H |
| 37 | Headset pressed state: icon bg #eaeaea, chevron bg #dadadc, radius ~6. | — | M |

## 5. User panel details visible in these shots (user_panel.rs)
| # | Discord 2026 | Emerald | Conf |
|---|---|---|---|
| 38 | Panel spans server list + sidebar (x ~10-412, ~400px), ~60px tall; subtitle switches from custom status to username on hover ("人間嫌い" vs "wolfsbany"). | Width `72+240-16` = 296px (user_panel.rs:38), height 56; no username-on-hover swap. The DM sidebar in 2026 is ~340px wide. | M |
| 39 | Order: avatar+name, mic+⌄, headset+⌄, gear. | avatar+name, mic, headset, gear. | H |

## New primitives needed
- `widgets::slider` (continuous, brand fill, knob), for both menus and the Voice page.
- Two-line menu item with a trailing chevron and submenu.
- Split icon button (icon + chevron) for the user panel.
- `theme::border_normal()` (~#dfdfe1 light) and a border on Secondary buttons.
- Modal shell (scrim + rounded bordered panel + header bar with X) for settings and the profile editor.
