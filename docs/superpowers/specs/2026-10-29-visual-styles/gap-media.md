# Composer, pickers, media: Emerald vs Discord 2026 screenshots

The screenshots use the light theme at 1.0 scale (DM rows are 44px apart, which is Discord's default). The user's chat font is larger than 16px (lines are 26px apart), so measurements of chat text come out too large. Measurements of panels and controls are taken at 1.0. Pixel values are measured from the PNGs (+-2px).

Confidence: H = seen in both the code and the screenshot. M = measured or inferred. L = a guess about Discord's exact token.

## 1. Image/GIF viewer modal (ao_clicar_em_imagens...): missing entirely (H)

Code: `emerald-ui/src/app/chat/media.rs:30-67` (`render_picture`) has no on_click for plain images. Only videos open, and they open in the browser (media.rs:51-59). Embed media also opens its link in the browser (`media/embed.rs:205-209`). A search for lightbox or viewer code finds nothing.

Discord 2026:
- A dark scrim covers the whole window (~85-90% black), dark even in the light theme. `theme::scrim()` is only 52% in the light theme (theme.rs:150), which is too light to reuse here. (M)
- The image is centered and fitted to the window, leaving ~105px at the top and ~110px at the bottom.
- Top left: the author's avatar (~40px circle), with the name (bold, ~15px, white) and below it the time (12px, muted): "Wolfsbany / 21:15". (H)
- Top right: a toolbar pill, ~182x40, radius ~8, with a dark translucent background and a 1px border. It holds five 20px icons, each on a ~36px hit area: zoom (magnifier with plus), forward (curved arrow), download, open in browser (box with arrow) and more (...). Next to it is a separate close button, 40x40, radius ~8, with an X. (H)
- Missing behavior:
  - open on click
  - close with Esc or a click outside
  - zoom, download, forward, open original
  - copy and save under "..."
  - arrows to move between a message's attachments
- Icons: `DOWNLOAD`, `FORWARD`, `MORE` and `CLOSE` already exist in icons.rs. Zoom and external-link icons do not.

## 2. Composer bar and quick buttons (botoes_de_acesso_rapido...) (H)

Code: `app/chat/mod.rs:237-268` builds a row of `[plus button] [TextInput box] [emoji button]`. The buttons sit OUTSIDE the bordered input. The input itself (`text_input.rs:261-279`) is rounded_lg, with a border, a surface_high background, px 16 and py 11 around a 22px line, so ~46px tall.

Discord 2026:
- One rounded bar (radius ~8, 1px subtle border, white in light theme, surface in dark) holds everything. The `+` sits inside the bar at the far left (x=457; the placeholder starts at x=493). Inside the bar at the right are Gift (Nitro gift), GIF, Sticker, Emoji and Apps (a 4-square grid). The bar is ~56px tall. (H for layout, M for height)
- Emerald has only the emoji button, and it sits outside the bar (`picker.rs:362-387`, 32x32, mt 7). The gift, GIF, sticker and apps buttons are missing. The GIF and sticker buttons should open the picker straight on their tab, but `open_composer_picker` always opens `PickerTab::Emoji` (picker.rs:100). (H)
- Icons:
  - The `+` is drawn with `icons::ATTACH`, a FILLED circle with a plus in it (assets/icons/attach.svg), which looks like 2018. Discord 2026 uses a plain stroked "+" with no disc.
  - The emoji button uses `icons::REACTION`, a smiley WITH a "+" badge. Discord's composer emoji button is a plain smiley; the add-reaction glyph appears only in message actions.
  - assets/icons has no gift, GIF, sticker or apps SVGs. (H)
- Discord's button icons are ~22-24px, with ~40px between centers (x = 1678, 1718, 1758, 1798, 1838). (M)

## 3. Expression picker panel (all three tabs) (H/M)

Code: `app/chat/picker.rs:221-312`.

| Item | Emerald | Discord 2026 | Conf |
|---|---|---|---|
| Size | WIDTH = 9*40+24 = 384, HEIGHT 420 (picker.rs:23-29) | ~495 x ~485 | M |
| Position | from the click point minus the panel size (picker.rs:82) | right edge near the composer's right edge, bottom ~10px above the composer | M |
| Panel radius, border, shadow | rounded_lg, border(), shadow_lg, surface_high | same family: radius ~8, white in light theme | M |
| Tabs | px 8, py 4, rounded_md, 14px medium, pt 8 (picker.rs:240-264) | pills ~28px tall, ~16px top padding, selected tab is a grey pill with radius ~8 | M |
| Tab label | "Figurinhas" (pt-BR.toml:118) | "Figurinha" (singular) | H |
| Search box | the shared `compact` TextInput (app.rs:316): 13px text, px 6, py 4, ~28px tall, base_lower background, grey border, no icon | ~462x38, radius 8, 16px text, magnifier icon at the left, 2px brand-blue border when focused | H |
| GIF placeholder | "Pesquisar no Tenor" (pt-BR.toml:115) | "Buscar Klipy" (Discord switched its GIF provider to Klipy) | H |
| Emoji tab extra | none | skin-tone (diversity) hand button to the right of the search box | H |
| Left rail | none | ~48px category rail: a clock (frequently used), one icon per server (with a lock badge when locked), then the Unicode categories; clicking scrolls the list | H |
| Section headers | text_xs, semibold, muted, plain (picker.rs:317-323, expressions.rs:258-265) | ~14px semibold in the default text color, a leading icon (clock or server icon), a trailing chevron; collapsible and sticky | H |
| Footer | only on the Emoji tab (picker.rs:266), background base_lowest | Emoji tab: smiley icon, the emoji and ":sob:". Sticker tab: sticker icon, a preview, "Wave" / "de Wumpus Beyond", and the pack icon at the right | H |

### Emoji tab (aba_de_emojis...)
- Cells: Emerald uses CELL 40 and EMOJI 28 (picker.rs:25-26). Discord uses 9 columns on a 48px pitch with emoji of ~40px. (M)
- Emerald draws Unicode emoji as font text (`reactions.rs:213-218`), so they come out as Segoe UI Emoji on Windows and Apple Color Emoji on macOS. Discord draws Twemoji images everywhere, and the screenshot shows Twemoji. The difference is very visible. (H)
- Locking:
  - `custom_emoji` (picker.rs:159-178) DROPS emoji the user cannot use: those from other servers without Nitro, and animated ones without Nitro.
  - Discord SHOWS them greyed out (desaturated, ~50% opacity) with a small lock overlay. It puts a lock badge on locked server icons in the rail and shows a Nitro upsell strip (a purple gradient banner with a lock) inside the list.
  - Clicking a locked emoji should do nothing or show the upsell, not insert it. (H)
- Animated custom emoji are drawn as stills in the picker: `emoji_image` uses `avatar_image` (reactions.rs:209). (M that Discord animates them there)

### Sticker tab (aba_de_figurinhas...)
- The "Utilizados com frequência" (recent stickers) section is missing. (H)
- The same locking gap applies: `usable_stickers` (expressions.rs:134-162) leaves out other servers' stickers when they cannot be used here. Discord lists every server and greys out the unusable ones. (H)
- Sections: Discord's headers show the server icon, the name and a chevron. Discord's own packs come after the servers, and that order already matches. (H)
- Sticker size: Emerald uses STICKER 80 in 84px cells, 4 columns, gap 4 (expressions.rs:25-26, 266-285). Discord uses ~96px stickers in ~108px cells, with a hover background of radius ~8. (M)
- There is no footer preview when hovering a sticker. (H)

### GIF tab (botoes_de_acesso...)
- With an empty search, Emerald lists the trending GIFs directly (rest/expressions.rs:22, `/gifs/trending-gifs`). (H)
- Discord instead shows a landing grid of category tiles in 2 columns: (H)
  - "Favoritos", a brand-blue tinted tile
  - "GIFs em alta", with a trending-arrow icon and a label
  - then trending search terms (hello, lol, love, happy birthday...)
  - Each tile is a GIF under a dark overlay, with a centered bold white label of ~16px. Clicking a tile searches that term.
- Discord probably gets these categories from `/gifs/trending`; check the bundle. (L)
- Favorites (starred GIFs, kept in user settings) are missing, and so is the star on hover. (H)
- Previews are STATIC: `render_gif_tab` uses `this.avatar_image(url)` (expressions.rs:315), and `Gif::image_url` prefers the `preview` still. Discord plays every GIF in the grid. (H)
- Grid: Emerald uses a fixed 2 columns, rows of 120px including 8px bottom padding, cells (384-32)/2 = 176 wide, cover crop and rounded_md (expressions.rs:24, 300-324). Discord's landing tiles are ~230x110 with 12px gaps and radius ~4-6. Its search results are a masonry layout (variable heights, aspect ratio kept). (M)

## 4. Site embed and image hover options (embed_de_site...) (H/M)

Code: `app/chat/media/embed.rs:10-118`. Model: `emerald-discord/src/model/message/embed.rs`.
- Footer: Emerald shows only the footer text (embed.rs:24,114). Discord shows a footer icon (the X logo, ~20px round) followed by "X - Hoje às 15:48" (the footer text plus the formatted embed timestamp). The model needs new fields for this: `EmbedFooter` has no `icon_url`, `Embed` has no `timestamp`, and `EmbedAuthor` has no `icon_url` or `url`. (H)
- Author line: Discord shows "bombardeo cósmico (@bonbardero_6565)" at ~14px semibold in text_strong, which matches. The author icon is missing because of the model. (M)
- Card:
  - Discord: in the light theme, a white card with a 1px border, a 4px left bar (blue/brand for X) and radius ~4-6.
  - Emerald: rounded_md (6) with a base_low background (fbfbfb in light), which is close.
  - Emerald's max_w is 520 (embed.rs:75); Discord's is ~432 for rich embeds. (L)
- Video thumbnail: Discord's play button is a ~40-48px dark circle; Emerald's is a 56px scrim circle (media.rs:223-247). Discord plays video embeds inline, while Emerald opens the browser. (M)
- Hovering an attachment image:
  - Discord shows a small floating toolbar at the image's top right (~64x32 white pill, radius ~6, shadow). On your own messages it has a pencil (edit the attachment's alt text or spoiler) and a trash can (remove this attachment).
  - Emerald has no hover actions on pictures (media.rs:30-67), only an alt tooltip and the "ALT" badge.
  - Discord's hover actions on other people's images are probably different, but this screenshot does not show them. (H for the own-message case, L for others)
- The same screenshot shows the message hover bar with 3 quick-reaction emoji before the icons. That is outside this scope; pass it to whoever reviews message actions. (M)

## 5. Smaller items
- Several image attachments stack one per line (list/parts.rs:91-93); Discord arranges them in a mosaic grid. These screenshots do not show it. (L)
- The "GIF" and "ALT" `badge()` (media.rs:250-262) uses scrim plus text_xs bold, which broadly matches Discord's style. (M)
