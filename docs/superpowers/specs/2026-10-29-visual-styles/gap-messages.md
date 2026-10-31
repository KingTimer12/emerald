# Messages and profiles: Discord 2026 screenshots compared with Emerald

Screenshots: `D:\app\emerald\themes\2026\` (light theme, about 100% scale, window 1874px wide).
Pixel numbers come from the screenshots and are estimates (±2px). "Certain" means the
code shows it; "inferred" means it is read off the image.

Files involved:
- Message rows: `emerald-ui/src/app/chat/list.rs`, `emerald-ui/src/app/chat/list/parts.rs`
- Hover toolbar and context menu: `emerald-ui/src/app/chat/actions/menu.rs:57-138`
- Avatar and decoration: `emerald-ui/src/app/widgets.rs:39-108`, `emerald-ui/src/app.rs:495-517`
- DM side profile: `emerald-ui/src/app/profile.rs:240-331`
- Full profile modal: `emerald-ui/src/app/profile/modal.rs`, tabs and note: `profile/social.rs`
- Models: `emerald-discord/src/model/profile.rs:9-45`, `emerald-discord/src/model/user.rs:8-27`
- Colors: `emerald-ui/src/theme.rs`

---

## 1. Message rows (both hover screenshots)

What matches (certain): a 40px avatar (`list.rs:301`), 16px left padding (`px_4`) and a
16px gap from the avatar to the text (`gap_4`, `list.rs:294`), so the text starts at 72px.
That matches the screenshot (avatar x441 to 481, text x497). Lines in a group are 26px
apart, which matches `py(2)` plus a 22px line. Hover background is `theme::hover()`,
#97979f at 12%, which looks right.

Differences:

| # | Discord 2026 | Emerald | Where | Confidence |
|---|---|---|---|---|
| M1 | The timestamp for today is just the time, **"21:22"** (12px, muted). | It says **"Hoje às 21:22"** / "Today at 21:22", from `time::header`. | `list.rs:331` uses `msg.time`; `time.rs:88-94`; `pt-BR.toml:360` | Certain |
| M2 | A **guild tag pill** sits after the name: a small icon plus "ZLDA", about 18px tall, radius about 4, a light grey tinted background, 11 to 12px semibold text. It also shows in the popout, the panel and the modal next to the username. | Not there. `User` has no `primary_guild` field. | `model/user.rs:8-27`; name row at `list.rs:317-332` | Certain |
| M3 | **Display name styles** (Nitro): the author name uses a custom font (a blackletter or serif face for "gato calculos") and, on hover, a gradient or effect (orange gradient on the hovered row in screenshot 1). | The name is plain `FontWeight::MEDIUM` in the role color or `text_strong`. `User` has no `display_name_styles`. | `list.rs:322-325`, `chat/mod.rs:366-369` | Certain that it is missing; the font and effect are inferred |
| M4 | **Clicking the name** opens the user popout. | The name has no click handler; only the avatar does. | `list.rs:322-325` vs `list.rs:296-308` | Certain |
| M5 | Clicking the avatar opens the **popout** (a small card beside the message, see section 3). | It opens the **full profile modal** right away (`open_profile` sets `Overlay::Profile`). | `list.rs:305-307`, `profile.rs:347-367` | Certain |
| M6 | A grouped (follow-up) message shows a **hover timestamp** in the gutter ("20:39" at x448, 11px muted, right-aligned in the 72px gutter). | Follow-up rows draw only `pl(72)` and the body. There is no gutter time. | `list.rs:265-278` | Certain |
| M7 | The name is 16px medium with a 12px timestamp, with about 8px between them. | `gap(6.)`, and the name uses the inherited text size. | `list.rs:319` | Gap: inferred, small |

## 2. Hover action bar

Screenshot (own message, Wolfsbany): 🔥 ⭐ ❤️ **|** add reaction, edit, forward, ⋯
Screenshot (friend, normal hover in the popout screenshot): 🔥 ⭐ ❤️ **|** add reaction, reply, forward, ⋯
Screenshot 1 (friend, **Shift held**, the expanded bar): Copy ID, copy link, create thread,
bookmark/save, pin, add reaction, reply, forward, ⋯, with no quick reactions.

| # | Discord 2026 | Emerald | Where | Confidence |
|---|---|---|---|---|
| H1 | A **forward** button sits before ⋯ on every message. | No forward button, although `icons/forward.svg` exists. | `menu.rs:114-135` | Certain |
| H2 | **Own messages show Edit instead of Reply.** | Reply and Edit are both shown on your own messages (`rights.reply` plus `rights.edit`). | `menu.rs:121-130` | Certain (both screenshots) |
| H3 | A **vertical divider** (1px, about 20px tall, `border-subtle`, 4px margin on each side) separates the quick reactions from the action icons. | No divider. | `menu.rs:97-114` | Certain |
| H4 | **Holding Shift expands the bar**: copy ID, copy link, create thread, save/bookmark, pin, mark unread, and so on. | Not supported. There is no Shift state, and there are no link, bookmark or ID icons in `assets/icons/`. | `menu.rs:57-138` | Certain |
| H5 | Buttons are about **28px apart**, with icons about 20px. The bar is about 32px tall, radius about 8, with a white surface, a 1px border and a soft drop shadow. | 32px buttons (`menu.rs:75,101`), a 2px pad, `rounded_lg`, a border, a `base_lower` background (#fbfbfb light / #323339 dark) and **no shadow**. | `menu.rs:86-96` | Pitch and shadow: inferred. Background: Discord uses `--background-surface-high` (white in light), while Emerald uses the chat color, so the bar blends into the row. |
| H6 | The reaction button icon is a "smiley +"; reply and forward are curved arrows. | Same set of icons apart from the missing ones above. | | Inferred, fine |

## 3. User popout (friend) — missing entirely

The screenshot shows a card about **300px wide** with radius about 8, placed next to the
clicked name or avatar and clamped to the window bottom. It is painted with the user's
**Nitro profile theme** (a dark brown gradient from `theme_colors`).
- Banner: about 105px tall. In its top-right corner sit two **circular icon buttons** (about 32px, dark translucent): add friend or friend status, and ⋯.
- Avatar: 80px with a 6px ring in the card color, at left 16, straddling the banner. It has a status dot and the decoration.
- Display name: 20px bold in the user's **custom font**.
- "username" plus the **guild tag pill**.
- **Badges in their own rounded, bordered container** (about 22px tall, radius 8, 4px padding, icons about 18px, "+3" overflow).
- **Mutual line**: three stacked 16px avatars, then "11 amigos mútuos • 6 servidores mútuos" (12px muted).
- Bio (14px), with links in `link()` color.
- A **"Coleção de jogos" card** (rounded 8, tinted background, game icons about 24px, "+29").
- A bottom **"Conversar com @name" message box** (about 44px, radius 8, with an emoji icon).

Emerald has **no popout** (grep finds no profile "popout"). `open_profile` goes straight
to the modal (`profile.rs:347`). Also missing everywhere:
- `UserProfileDetails` has **no `theme_colors`** (`model/profile.rs:9-22`), so profile cards always use `theme::base_lowest()` (`profile.rs:269`, `modal.rs:49`).
- No mutual friends or servers line on the card.
- No game collection widget.
- No quick-message field.

All certain.

## 4. Own popout

Plain theme colors (no Nitro theme):
- No banner image: a short top band with only a ⋯ button.
- Avatar 80px with a **custom-status thought bubble** ("人間嫌い") attached to the avatar's top right: a white bubble with a border, radius about 12, 12 to 14px text.
- Name 20px, plus a small "notes" icon.
- "wolfsbany • Mundano." (username • pronouns on **one line**, separated by a bullet), with the badge container on the same line.
- Bio.
- Game collection card.
- A full-width **brand "Editar perfil" button** with a pencil icon and **white text**.

Emerald:
- No popout.
- Custom status is a plain 14px text line (`profile.rs:165-169`), not a bubble.
- Pronouns are on a separate muted line (`profile.rs:306-311`, `modal.rs:118-123`).
- **Bug in the light theme:** `widgets::button` paints every kind with `text_strong()` (`widgets.rs:172`), which is #28282d in light (`theme.rs:70-72`). Brand, Positive and Danger buttons get dark text on blue, green or red; Discord uses white. Certain.

**Update (2026-10-30):** the user's newer screenshot
(`popover_ao_clicar_na_conta_propria_nao_abre_a_tela_de_config.png`) shows the own popover with
grouped menu cards instead of the single button: Edit Profile, status (side panel), Badges, Orbs,
then Switch Accounts (side panel) and Copy User ID. Built between plans 2 and 3 (`profile/own.rs`)
without Orbs (quests are excluded), Badges and the game collection card.

## 4b. Inbox popover (`popover_inbox.png`), left for plan 3

The Inbox is now a popover under the title bar button (`panels/inbox.rs`) with Unreads and
Mentions. Plan 3 adds its two other tabs, which need new requests found in the bundle:
- **Favorites** ("Favoritar mensagem" in the message menu): `GET /users/@me/saved-messages/bookmarks`.
- **Reminders** ("Lembretes", a reminder set on a message for later):
  `GET /users/@me/saved-messages`, `PUT` and `DELETE /users/@me/saved-messages/{channel}/{message}`.
Both writes only touch the user's own account. Find each body and query string in the
bundle (`GET_SAVED_MESSAGES`, `GET_BOOKMARKS`, `PUT_SAVED_MESSAGE`) before building.

## 5. Full profile modal ("Ver perfil completo")

Discord 2026:
- **About 960 x 800px**, radius about 12, with the whole modal painted in the Nitro theme gradient.
- **Two columns.**
- Left column: a card about 400px wide. Banner about 140px. **Avatar about 120px** with ring, status and decoration. Name about 24px in the custom font. "username" plus guild tag plus badges in a container.
- **Action row**: "Mensagem" (themed primary button, chat icon, about 32px tall, radius 8), an add-friend icon button and ⋯ (both about 32px square, radius 8).
- Bio and link, then sections with **12px semibold headings**: "Membro desde" with the date, **"Amigos desde"** with the date, and "Nota (visível apenas para você)" with an **inline italic placeholder** "Clique para adicionar uma nota" (no boxed input).
- Right column: tabs **"Mural · Atividade · 11 amigos mútuos · 6 servidores mútuos"** (14px semibold, 2px underline under the active tab, about 24px gap).
- Below the tabs, **widget cards** (radius about 12, a tinted surface 8% lighter than the theme, 16px padding, a ⋯ menu): "Jogo favorito" (an 88x116 cover with tag chips such as "Especialista" and "Meio que amo isso"), "Jogos em rotação", "Jogos que eu gosto" (a cover grid).
- Close: a **round 32px "X"** button at top right, sitting on the theme.

Emerald (`modal.rs`):
- **560px wide, single column** (`modal.rs:7,40`), banner 150, **avatar 96** (`modal.rs:8-9`), `rounded_lg`, a `base_lowest` background, and a border.
- Tabs **"About / Mutual servers (n) / Mutual friends (n)"**, with counts in parentheses, inside the same column (`social.rs:89-116`).
- **No Activity or Board ("Mural") tab and no game widgets.**
- **No action buttons** (Message, add friend, ⋯).
- No "Friends since".
- The note is a boxed `base_lower` input with a hint line under it (`social.rs:68-78`), not the inline italic placeholder.
- The bio sits under a "Sobre mim" heading (`modal.rs:126`). Discord 2026 shows the bio with no heading.
- Section headings are **14px semibold** (`profile.rs:401-404`); Discord uses about 12px semibold.
- Roles and connections are listed in About (`modal.rs:128-129`). In 2026 they are not in this view for a DM profile; that is inferred and may still be right for server profiles.
- Close button: a text glyph "✕" in a `scrim` `rounded_md` square (`modal.rs:80-88`). Discord uses a round button and `icons/close.svg` already exists.
- `self.effect` is drawn over the card (`modal.rs:78`), which is fine.

## 6. DM side profile panel (header toggle)

Discord 2026:
- The card fills the right column with 8px margins, radius about 8, and is **painted with the profile theme colors**: in light mode it is a tinted sand or grey gradient, with dark text kept readable.
- Banner about 110px, with **add friend and ⋯ buttons** in its top right.
- Avatar 80 with ring and decoration.
- Name in the custom font, then username, guild tag, the badge container on its own line, and the **mutual line** (stacked avatars, "11 amigos mútuos • 6 servidores mútuos").
- Bio with **no heading**, then the link.
- "Membro desde": a 12px semibold heading with a 14px date.
- The **game collection card**.
- A bottom secondary button "Ver Perfil Completo" (about 40px, radius 8, full width).

Emerald (`profile.rs:240-331`):
- `PANEL_WIDTH` 340 with `p_2`, and a `border_l_1` on the outer box (`profile.rs:252-258`). In the screenshot the column is about 320px wide and there is no left rule. Inferred.
- The card is `base_lowest` with a 1px border. **No theme colors.**
- Banner 120 (close to Discord). **No banner buttons.**
- Badges are inline after the username at 20px (`profile.rs:125-150`), with no container.
- Pronouns are on their own line.
- The custom status is a text line, and activities are sections.
- The bio has a "Sobre mim" heading (`profile.rs:313`).
- No mutual line and no game collection.
- The "View full profile" button: `ButtonKind::Secondary`, 38px, `rounded_lg`. That is fine.

## 7. Avatar decorations, nameplates, badges

- Decoration: 1.2 times the avatar, centered (`widgets.rs:96-108`). That matches Discord's frame. Certain it matches.
- The status dot on profile avatars is ringed with `base_lowest` (`profile.rs:197`). With theme colors, Discord rings it with the card's theme color. Inferred.
- Nameplates are drawn only in the member list (`members/row.rs:87-120`). The DM sidebar rows in the screenshots also show nameplates (the purple-gradient "gato calculos" row). That belongs to the sidebar and is out of scope here; noted.
- Badges: 20px (panel) or 22px (modal) icons in a row, with no container. Discord 2026 puts about 18px icons in a rounded pill container with an overflow "+N". Inferred from the screenshot.

## Ranked by visual impact

1. **No user popout.** A click on the avatar opens a big modal, and a click on the name does nothing (M4, M5, section 3).
2. **The full profile modal uses the old layout:** 560px and one column, versus 960px with two columns, action buttons, Mural/Activity tabs and widgets (section 5).
3. **No Nitro profile theme colors** (`theme_colors` is not parsed), so every profile, popout and panel is plain grey (sections 3, 5, 6).
4. **Guild tag pills and display-name fonts/effects are missing** in message headers and profiles (M2, M3).
5. **Timestamp reads "Hoje às 21:22" instead of "21:22"**, and **follow-up messages show no hover time in the gutter** (M1, M6).
6. **Hover bar:** no forward button, no divider, Reply wrongly shown on your own messages, no Shift-expanded bar, and the background blends with the chat with no shadow (H1-H5).
7. **The light theme draws dark text on brand, danger and positive buttons** (`widgets.rs:172`).
8. **Profile details:** headings 14px instead of 12px, a "Sobre mim" heading over the bio, pronouns on a separate line, badges without a container, the custom status not shown as a bubble, the note as a boxed input, and the close button drawn as the "✕" glyph.
