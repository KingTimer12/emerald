# Bundle findings for plan 3, part B (identity, profile theme, popout, widgets)

Source: the web client's chunks in `target/discord-intl/` (read-only grep, no network).
Main chunk: `web.057e7776e1c6251b.js` (below "web"). Other chunks are named by file.
Locale chunks: `26d528cd23025094.js` (en-US), `53088f6d9d7a0c0a.js` (pt-BR). Some newer
strings live in small per-feature locale chunks; those are named where used.
"Inferred" marks anything not read directly from code.

**No CSS was downloaded** (`target/discord-intl/` has no `*.css`). Class names are
known from the CSS-module maps in JS, but no rule bodies (paddings, radii, gradients).
Every layout number below comes from JS constants.

**Already parsed by Emerald:** `User` has id, username, global_name, avatar, bot,
premium_type, avatar_decoration_data, collectibles. `Profile` has user, user_profile (bio,
pronouns, banner, accent_color, collectibles), badges, connected_accounts, guild_member,
guild_member_profile, mutual_guilds, mutual_friends_count. `Relationship` has id, type,
user, nickname, user_ignored, note. `Role` has id, name, color, position, permissions,
mentionable. Missing fields are listed per question.

---

## 1. Guild tag (primary guild)

**Field.** `user.primary_guild` (not `clan`). Normalizer (web, module `47537`):

```js
function r(e){return null==e?null:"identityGuildId"in e?e:{identityGuildId:e.identity_guild_id,
  identityEnabled:e.identity_enabled,tag:e.tag,badge:e.badge}}
```

Shape: `{ identity_guild_id: snowflake|null, identity_enabled: bool|null, tag: string|null,
badge: string|null }`. It comes with every user object (READY, messages, members,
`USER_UPDATE`). A user update with `primary_guild == null` does not clear a known one
(`null!=e.primaryGuild&&null==t.primary_guild: t.primary_guild=e.primaryGuild`).
**Missing in Emerald:** `User.primary_guild`.

**When shown** (`685073.Zo` / `.Wb`): only if `identity_enabled` is true and both
`guildId` and `tag` are set, and the member is not automod-quarantined in the
context guild:

```js
function E(e){return null!=e&&e.identityEnabled?{guildId:e.identityGuildId,tag:e.tag,badge:e.badge}:{}}
function h(e,t,n){...{tag:d,guildId:c}=E(void 0!==n?n:s?.primaryGuild);return null!=c&&null!=d&&!o}
```

`o` = the member has the flags `AUTOMOD_QUARANTINED_BIO` and similar (`229527.TR`).

**Badge URL** (web, `685073.gC`):

```js
return`https://${i}/clan-badges/${e}/${t}.png?size=${r}`   // e = guild id, t = badge hash
let E={12:16,14:16,16:16,24:24,32:32,36:40}                 // display size -> ?size=
```

So `https://cdn.discordapp.com/clan-badges/{identity_guild_id}/{badge}.png?size=16` for
the usual 12/14/16 px badge. On image error a placeholder span is drawn.

**Chip** (web, module `534400`): a span with `[badge img][tag text]`. Default text
`text-xs/semibold`, color `text-default`, badge `SIZE_14`, inline. An aria-only
"Server Tag: {tag}" and a copy-only `" [TAG], "` text sit beside it.
CSS-module classes (chunk `234017…`): `chipletContainerInner`, `chipletContainerInline`,
`messageChipletContainerInner`, `messageText`, `messageBadge`, `badge`, `badgePlaceholder`,
`tagText`, `clickable`; rule bodies not available (inferred: small rounded pill, subtle
background). Clicking opens a guild-profile popout unless `disableGuildProfile`.

**Where shown (call sites found):**
| Place | Code | Size / variant |
|---|---|---|
| Message header (cozy and compact) | `1391cbb5…`, `df7c6c44…`: `j.Cw,{primaryGuild:l.primaryGuild,userId:n.author.id,contextGuildId:Q,…badgeSize:b?SIZE_12:void 0}` | 14 (12 compact) |
| Member list row | web `el` memo: `!s&&(0,r.jsx)(k.Ay,{primaryGuild:l?.primaryGuild,…})`, hidden by `hideClanTag` | default |
| DM list row (1:1 DMs only) | web: `eV=eM&&l?.primaryGuild!=null` then `ep.Ay{…inline:!0,disableGuildProfile:!0}` | default |
| Friends list row | `a02eab69…` `FriendListRow`: `ed.Ay,{userId:t.id,disableGuildProfile:!0}` | default |
| Voice participant | `a1764c83…` | default |
| Profile popout name row | `32b2728c…`: `E.Ay,{primaryGuild:o,…textVariant:"text-xs/normal"}` | default |
| Full profile modal | `27027732…`: `badgeSize:SIZE_16,textVariant:"heading-md/semibold"` | 16 |

Strings (small locale chunks, en `7ecb5714…`, pt-BR `09ef0904…`): `fw2p/x`
"Server Tag: {tag}" / "Tag do servidor: {tag}"; `4cGHIl` "[{tag}]" (copy text).

---

## 2. Display name styles

**Field.** `user.display_name_styles = { font_id, effect_id, colors: [int RGB] }`
(web `945096.mT`: `{fontId:e.font_id,effectId:e.effect_id,colors:e.colors}`). Guild
members carry their own `display_name_styles` too (member normalizer lists
`display_name_styles:h`), and it wins over the user's:
`E?.displayNameStyles??u?.displayNameStyles` (`609425`). **Missing in Emerald:** both.

**Fonts** (enum `559949`, names from small locale chunks: en `f91d9f20…`, pt-BR `9893fbdf…`):

`id ENUM = picker name (en / pt when different)`:
11 DEFAULT = gg sans · 1 BANGERS, 2 BIO_RHYME, 5 COMPAGNON, 9 RIBES = no name (not
offered) · 3 CHERRY_BOMB = Sakura · 4 CHICLE = Jellybean / Geleia · 6 MUSEO_MODERNO =
Modern / Moderno · 7 NEO_CASTEL = Medieval · 8 PIXELIFY = 8Bit · 10 SINISTRE = Vampyre /
Vampiro · 12 ZILLA_SLAB = Tempo · 13 PLAYPEN_SANS = Monkey Bars · 14 ORBITRON = Mainframe ·
15 NEW_ROCKER = Headbang · 16 KALAM = Journal · 17 HEXAGON = `ALTlzk` (en only:
"Vice City Theme").

Font ids map to CSS classes (`73392`), not to family strings in JS, so the real
`font-family` is in CSS that was not downloaded. **Inferred** open-licence matches from
the enum names (all on Google Fonts, OFL): Bangers, BioRhyme, Cherry Bomb One, Chicle,
Compagnon (Velvetyne, OFL), MuseoModerno, Neo Castel (inferred; check the licence),
Pixelify Sans, Ribes (inferred), Sinistre (Velvetyne, OFL), Zilla Slab, Playpen Sans,
Orbitron, New Rocker, Kalam. Hexagon is unknown. The fonts `CHERRY_BOMB` and `CHICLE`
render bold text at `boldFontOpacity` (0.9 in the DM list).

**Effects** (enum `508425`; names en/pt from the same chunks):
| id | enum | Name en / pt | Colors used | Min contrast |
|---|---|---|---|---|
| 1 | SOLID | Solid / Sólido | 1 | 3 |
| 2 | GRADIENT | Gradient / Gradiente | 2 | 2.5 |
| 3 | NEON | Neon | 1 | 3 |
| 4 | TOON | Toon / Desenho | 1 | 3 |
| 5 | POP | Pop | 1 | 3 |
| 6 | GLOW | (no name) | 2 (like gradient) | 2.5 |
| 7 | PRISM | Prism | 5 | 2.5 |
| 8 | GUMMY | Gummy | 4 | 3 |

(`u(e)`: GRADIENT→2, GUMMY→4, PRISM→5, else 1.)

**How colors apply** (web `660184`, component `UserNameWithEffects`):
1. Each color is pushed to the contrast ratio above against `BACKGROUND_BASE_LOW` (TOON:
   `#333`), desaturated if the user has that on (`p.lZ`).
2. GRADIENT/GLOW: `--custom-display-name-styles-gradient-stops` = `c0 10%, …, cN 90%`.
3. PRISM: `--custom-display-name-styles-prism-stops` = colors evenly spaced 0–100% (first
   repeated at the end), cycle `max({nameLength}ch, 38px)`.
4. SOLID/NEON/TOON/POP/GUMMY use the first color and derive a palette (`945096.$0`):
   ```js
   light1:l*1.2, light2:l*1.6, dark1:l*0.6, dark2:l*0.2   (hsl.l, clamped 0..1)
   toonStroke: l=max(.12, .4*l);  neonStroke: s=min(1,1.2*s), l=min(.6, l+.1)
   ```
   These go in `--custom-display-name-styles-{light-1,light-2,dark-1,dark-2,toon-stroke,neon-stroke}-color`.
5. The text-shadow and stroke rules live in CSS (not available). GUMMY splits the
   name into letter spans with per-letter colors.

`effectDisplayType`: PLAIN (0) shows no effect, STATIC (1) shows it frozen, ANIMATED (2)
animates unless reduced motion is on. There is a user setting
`displayNameStylesEnabled` (`824994`); when it is off, no styles are shown.

**Where shown:**
- Message header: **font** everywhere; **effect** only in private channels
  (`em=a?.isPrivate()&&null!=el`); in guilds the name keeps its role color.
- Member list: in a guild only the font (role-colored, `displayNameStylesFont`); full
  effect only without a guild, e.g. group DM members (`_=null==o&&null!=c`).
- DM list row: full effect, ANIMATED on hover/selected, else PLAIN. Friends list: STATIC.
  Popout and full profile: full effect (`inProfile`).

---

## 3. Profile theme colors

**Field.** `user_profile.theme_colors: [primary, accent]` as int RGB, and the same on
`guild_member_profile.theme_colors` (web, the profile store):
`themeColors:t.user_profile?.theme_colors`. **Missing in Emerald.** Also missing:
`popout_animation_particle_type`, `widgets`, `guild_badges`, `premium_since`,
`premium_guild_since`, `legacy_username`, `profile_effect` and other collectibles.

**Picking the two colors** (web `101928`): used only when the profile `canEditThemes`
(Nitro). Otherwise there is no custom theme.

```js
C=n?.getPreviewThemeColors(E),O=C?.[0]??(0,i.LX)(S),R=C?.[1]??(0,i.LX)(N)
```

A missing color falls back to the avatar's two dominant colors (`d.rh(avatarURL)`).

**Light or dark card** (`543699.tM` with `317097.OK`):
```js
function E(e){return 1-(.299*(e>>16&255)+.587*(e>>8&255)+.114*(255&e))/255}
function d(e){return null==e?null:E(e)>.5?DARK:LIGHT}     // applied to the PRIMARY color
```
So a perceived brightness under 50% gives a dark card. With the setting
`syncProfileThemeWithUserTheme` on, the client theme is used instead.

**CSS variables** (web `920601`):
```js
"--profile-gradient-primary-color": hsla(primary),
"--profile-gradient-secondary-color": hsla(secondary),
"--profile-gradient-overlay-color": overlay,
"--profile-gradient-button-color": hsla(wg(primary)),
"--profile-gradient-modal-background-color": hsla(v2(primary, secondary, theme)),
...Zk({enabled:true, primary, secondary, isDarkTheme, textMixAmount:25})
```
- **Overlay** (design tokens): `PROFILE_GRADIENT_OVERLAY` = black α0.6 (dark card) /
  white α0.6 (light); `…_SYNCED_WITH_USER_THEME` α0.8; `SECTION_BOX` α0.45;
  `NOTE_BACKGROUND` α0.3; role pill `PRIMARY_660`/white α0.5; divider 0.24 / 0.12.
- **Button color** `wg`: starts from the primary color and steps it with chroma-js
  `darken()`/`brighten()` (up to 100 steps) until its contrast with `#ffffff` is between
  **7 and 10** (`HighContrastText:7`, tolerance 3).
- **Modal background** `v2`: mix primary and secondary 50/50 in Lab, then clamp HSL
  lightness to `[0, 0.1]` on a dark card or `[0.8, 1]` on a light one.
- **Text and base colors** `Zk`: `l`/`o` = lighter/darker of the two by luminance (if
  `o` has no hue, `o = l`). Dark: base = `o`(s=1,l=.05), text = `l`(s=1, l=.8 if hue in
  200–300 else .95). Light: base = `l`(s=1,l=.9), text = `o`(s=1, l=.25 if hue in 200–300
  else .15). Output `--custom-theme-{base,text}-color-{dark,light}`, text amount 25%,
  base amount 50%.
- With no theme, the defaults are primary and secondary = `var(--background-surface-high)`,
  overlay transparent, button `var(--background-mod-subtle)`.
- **Gradient geometry: not in JS.** Inferred from the live client: a vertical
  `linear-gradient(primary, secondary)` on the card, with the overlay color laid on
  top. The class is `custom-user-profile-theme`.

**No banner:** the banner area is filled with `primaryColor` when it is set. Otherwise it
uses the avatar's dominant color, adjusted (`0c2828db…` `F`, `O.A(n?.primaryColor??_)`).
Banner images are requested at `size=480` in the popout and preloaded at `size=600`.

---

## 4. User popout

**Request when opened** (web `8442243` preload, then `903209` and `g`):
```js
{type:"popout",withMutualGuilds:n.id!==a.id,withMutualFriends:!n.bot&&n.id!==a.id,guildId:s,channelId:d}
query:{type:t,with_mutual_guilds:n,with_mutual_friends:r,
       with_mutual_friends_count:i&&(null==r||!r),guild_id:o,connections_role_id:d,join_request_id:c}
```
So: `GET /users/{id}/profile?type=popout&with_mutual_guilds=true&with_mutual_friends=true&guild_id={g}`.
`with_mutual_friends_count` goes only when the list is not asked for (the modal asks
count only: `type:"modal",withMutualFriendsCount:!0,withMutualFriends:!1`). The popout
never sends `connections_role_id`/`join_request_id`; `guild_id` is dropped for "@me".
No refetch within 60 s, nor after a 404/429 within 60 s (`Date.now()-fetchEndedAt>=6e4`).
Afterwards it fetches the apps of `application` widgets and of the activity card. The
response carries `mutual_friends: [user]` (sorted by username) and `mutual_friends_count`.

**Mutual line** (chunk `aa466a2a…`): up to 3 friend avatars (16 px) and
"N Mutual Friends", then a dot divider, then "N Mutual Servers". The guild icons show
only when there are no mutual friends. Text is `text-xs/normal` when both parts show,
else `text-sm/normal`. Each part opens the full profile on that tab.
`5s9jl+` "{count} Mutual Friend(s)" / "{count} amigo(s) mútuo(s)"; `eE3oep` "{count}
Mutual Server(s)" / "{count} servidor(es) mútuo(s)". As tab titles, count unknown →
`0mTJ3j` "Mutual Friends"/"Amigos em comum", `4lTDZq` "Mutual Servers"/"Servidores em
comum"; count 0 → `n9g3ay` "No Mutual Friends", `jpY0X5` "No Mutual Servers"/"Sem
Servidores em Comum".

**"Message @name" box** (chunk `1760eb9c…`; placeholder `0ZQw/X` "Message @{name}" /
"Conversar com @{name}"). On Enter (`448613.p`):
```js
R=h?await l.A.openPrivateChannel({recipientIds:t,location:S}):…   // POST /users/@me/channels
… return i.A.sendMessage(p.id,e,T,{location:A.Hx.USER_PROFILE,nonce:g})
```
So it opens or creates the DM (location = newest analytics location), then makes a
normal `POST /channels/{dm}/messages` with a nonce kept across retries. The location
is `"user_profile"` (inferred: the `Hx` enum has `USER_PROFILE="user_profile"`). With
`openChannel:true` it also navigates to the DM, then the popout closes. Empty text is
not sent. The box shows only when `570287.A(userId,guildId)` holds; otherwise a button
is shown in its place.

**Banner buttons** (chunk `6709797b…`, in this order):
1. Mod view shield: only for moderators in a guild; tooltip `kj3tz2`.
2. Friend button (`193738`; not for bots, self, blocked). None → `w5uwoI` "Add Friend" /
   "Adicionar amigo" → `addRelationship({userId,context:{location}})`. Friend → icon
   `G7jMpU` "Friend"/"Amigo"; outgoing `s/+byI` "Outgoing Friend Request" / "Pedido de
   amizade enviado"; incoming `6QQCQ+` "Incoming Friend Request" / "Pedido de amizade
   recebido". These three open menu `Jszi3G` "Friend Actions" / "Ações do Amigo"
   (remove/cancel friend + two items not resolved).
3. "More" `UKOtz+` "More"/"Mais", menu `user-profile-overflow-menu`, groups:
   [`view-profile` `+Xp3hq` "View Full Profile"/"Ver Perfil Completo", `share-profile`
   `sFN1/M` "Share Profile"/"Compartilhar perfil", a guild item, `shop-this-look`, invite
   to server, Nitro referral] · [Ignore/Unignore, Block/Unblock, `report-user` `A1MM3D`
   "Report User Profile"/"Denunciar Perfil do Usuário" (danger)] · [`copy-user-id`
   `/AXYnE` "Copy User ID"/"Copiar ID do usuário"; under an experiment a submenu
   `QvQeLv` "Copy User Info" with ID, profile link, avatar and banner URL].

**Popout sizes** (`207634`): `POPOUT {avatarSize:80, avatarOffsetX:10, avatarOffsetY:4,
bannerWidth:300, bannerHeight:105, themePadding:2}`. Avatar 80 has a 6 px stroke
(`SIZE_80:{size:80,status:16,stroke:6,offset:4}`). The embed variant also sets
`--custom-user-profile-content-inset: 16px`.

---

## 5. Widgets ("Board" / "Mural"), games, Activity

**Source:** the `widgets` array in the profile response (`t.widgets?.map(k)`). There is
no separate GET. Saving is `PUT /users/@me/widgets {widgets:[…]}`. **Missing in Emerald.**

**Shapes** (normalizer `k`; types `540185`):
```js
case CURRENT_GAMES|FAVORITE_GAMES|PLAYED_GAMES|WANT_TO_PLAY_GAMES:
  e.data.games.map(e=>({gameId:e.game_id,comment:e.comment,tags:e.tags}))   // uniq by gameId
case APPLICATION:   {applicationId:e.data.application_id}
case PERSONAL:      {header:e.data.header, sections:[{type:COVER,title,subtitle,image}|{type:FIELDS,fields:[{title,description,image}]}]}
case CLIPS_GALLERY: clips[{id,file_id,game_id,title,tags,video_url,thumbnail_url,…}]
```
Widget wire form: `{ id, data: { type: "current_games" | "want_to_play_games" |
"played_games" | "favorite_games" | "application" | "personal" | "clips_gallery", … } }`.
Images are `{file_id, width, height, is_animated}`. Tags are strings such as `love_it`,
`casual`, `expert`, `lfg`, `noob` and `open_to_play` (enum `761915`).

Widget titles (`229231`):
favorite_games `Rpf6Ak` Favorite Game / Jogo favorito · current_games `zs6NsE` Games in
Rotation / Jogos em rotação · want_to_play_games `I509Dl` Want to Play / Quero jogar ·
played_games `QTq6Pf` Games I Like / Jogos que eu gosto · application = app name ·
personal `AVkYMx` Personal / Pessoal · clips_gallery `zY8Ghg` Clips Gallery / Galeria de
clipes.

**Game data:** `GET /games?game_ids=…` (batches of 20; `920495`). It returns game
records with `media.{icon,cover,banner,artwork}`, each `{type:"hash"|"url",value}`.
- Cover URL: `getGameAssetURL` → `S({endpoint:APPLICATION_ICON,path:"app-icons",…})`. So
  `https://cdn.discordapp.com/app-icons/{game_id}/{media.cover.value}.{webp|png}?size=…&keep_aspect_ratio=true`.
  The exact query string comes from the shared builder `S`; inferred to match other
  CDN icon URLs.
- A `type:"url"` asset goes through the media proxy (`534573.Ec`).
- `application` widgets use `GET /applications/public?application_ids=…`.

**Full profile tabs** (modal V2, `c3173946…`): the order is shown below.
```js
(g||f)&&d.push({text:laViwx,section:WIDGETS}), d.push({text:chq59f,section:ACTIVITY}),
(c||!c&&o)&&d.push({text:"7lZ31J",section:WISHLIST}), t.id!==n?.id&&s&&(MUTUAL_FRIENDS, MUTUAL_GUILDS)
```
Board `laViwx` "Board" / "Mural" (only with widgets) · Activity `chq59f` "Activity" /
"Atividade" · Wishlist `7lZ31J` "Wishlist" / "Lista de desejos" · then mutual friends
and mutual servers (not on your own profile).

**Activity tab source:** `GET /content-inventory/users/{id}/outbox`. The body has
`entries[]` (recent games and similar). The entry shape was not read; inferred to match
the content-inventory feed entries. The tab also shows live activities from presence.

---

## 6. Role icons

**Fields:** `role.icon` (hash) and `role.unicode_emoji` (`unicodeEmoji`). **Missing in
Emerald's `Role`.**

**URL** (web `376304`):
```js
let o=`${location.protocol}//${CDN_HOST}/role-icons`; … s=QB?"webp":"png", u="size="+kr(t*devicePixelRatio), _="quality=lossless"
`${o}/${n}/${i}.${s}?${u}&quality=lossless`
```
So `https://cdn.discordapp.com/role-icons/{role_id}/{icon}.webp?size={px*dpr}&quality=lossless`.
A unicode emoji is drawn as that emoji image instead.

**Gate:** the guild has the `ROLE_ICONS` feature, or the role has
`tags.subscription_listing_id` (`fm`).

**Which role:** the member's `iconRoleId`, which is the highest-positioned role with
`icon` or `unicode_emoji` (web: `(null!=t.icon||null!=t.unicodeEmoji)&&(null==r||zA(t,r))&&(r=t)`).

**Where:** message header after the name, default size 20
(`eY.Xx({guildId,roleId:F.iconRoleId})` in `7174e795…`, `808130cd…`, `2ce363d6…`);
member-list group headers at 16 px (`a3addf0f…` `282006`: `Xx({roleId:l,guildId:j,size:16})`);
role pills in the profile.

Tooltip is the role name. Alt text `9+YWrE` "Role icon, {name}" / "Ícone de cargo, {name}".

---

## 7. "Friends since"

`relationship.since` (ISO timestamp), present on READY relationships and on
`RELATIONSHIP_ADD` and `RELATIONSHIP_UPDATE` (`since:e.since`). It is shown only when the
relationship type is FRIEND:

```js
X.A.getRelationshipType(t)===FRIEND?X.A.getSince(t):null … toLocaleDateString(t,{month:"short",day:"numeric",year:"numeric"})
```

It appears in the full profile, after "Member Since", as a section headed `wlTO8v`
"Friends Since" / "Amigos desde". **Missing in Emerald's `Relationship`.**

---

## 8. Full profile modal layout

CSS is not available. From JS (`207634`): `MODAL_V2` (current) avatar 120 (stroke 8),
avatarOffsetX 24, offsetY 0, banner **400 × 140**; legacy `MODAL` avatar 120, banner
600 × 210; `SIDEBAR` (DM side panel) avatar 80, banner 340 × 120. The V2 modal has a collapsible right panel. It opens expanded when the window is at least
929 × 550 (`window.innerWidth>928`, `(min-width: 929px) and (min-height: 550px)`) and
animates in 300 ms. The name heading uses `heading-lg/bold` (`sm`) or
`heading-xl/semibold` (`lg`). Tab styling, modal width and radii are in CSS:
**not found**.
