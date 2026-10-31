# Bundle findings for plan 2 (friend request note, guild header rows, Active Now, member-list Activity, bell)

Source: the web client's chunks in `target/discord-intl/` (read-only grep, no network).
Main chunk: `web.057e7776e1c6251b.js` (below "web"). Locale chunks used to resolve
message keys: `26d528cd23025094.js` (en-US), `53088f6d9d7a0c0a.js` (pt-BR).
"Inferred" marks anything not read directly from code.

---

## 1. Friend request with a personal note

**Route.** `POST USER_RELATIONSHIPS()` = `/users/@me/relationships` (web):

```js
USER_RELATIONSHIPS:function(){let e=...:tk.ME;return`/users/${e}/relationships`}
sendRequest(e){let{discordTag:t,context:n,captchaPayload:i,note:a,errorUxConfig:s=0}=e,[l,o]=t.split("#");
return r.Bo.post({url:I.Rsh.USER_RELATIONSHIPS(),body:{username:l,discriminator:parseInt(o),note:a,...i},context:n,...
```

**Body:** `{ username, discriminator, note?, ...captcha }`.
- `discriminator: parseInt(undefined)` is `NaN`, which `JSON.stringify` sends as `null`. So the
  wire body is `{"username":"x","discriminator":null}`, the same as Emerald today.
- `note` is `undefined` when empty, so it is **left out** of the JSON.

**Max length and normalization.** Module `237309` (web) is the "add friend by tag" helper:

```js
let s=120,l=function(e){let{discordTag:t,note:n,location:l,...}=e;!(t=t.trim()).includes("#")&&t.startsWith("@")&&(t=t.substring(1));
...if(null!=n){if(n.length>s)return Promise.reject(a.intl.string(a.t.ckHwck));n=n.replace(/\n/g," ").trim()}
return i.A.sendRequest({discordTag:t,context:{location:l},note:""!==n?n:void 0,errorUxConfig:o})
```

- Max **120** characters (`XG = 120`), checked before the trim.
- Newlines become spaces, then the note is trimmed, and an empty note is not sent.
- A server error on the note shows up as `e.body.note`, and the client then shows `ckHwck`.

**UI (add-friend form, web, `EO` component):**
- Form `name:"add-friend"`, username input `maxLength:37`.
- When the experiment is on, a second field shows the placeholder `Yi6Mpu` and `maxLength:Ef.XG`
  (120). Pressing Enter in it is blocked. The counter shows `120 - len` and turns
  `text-feedback-critical` at ≥120.
- The hint below it is `UtfQNw`.
- Submit sends `note: c && u.length>0 ? u : void 0` with `location:"Add Friend"`.
- **Gate:** the user experiment `"2026-03-friend-request-message"`, with default `{enabled:false}`
  and variation 1 `{enabled:true}` (module `189883`).
- Strings:
  - `Yi6Mpu` "Personalize your request (Optional)" / "Personalize sua solicitação (opcional)"
  - `UtfQNw` "What you write here will also show up in your DM if you become friends."
  - `ckHwck` "Notes can only contain plain text and must be under 120 characters." /
    "A mensagem deve conter apenas texto e menos de 120 caracteres."
  - `Rtl1Ep` "Success! Your friend request to **{discordTag}** was sent." /
    "Sucesso! Seu pedido de amizade foi enviado para **{discordTag}**."

**Receiving a note.**
- The relationship object has `note`. READY `relationships[]` carries it:
  `null!=e.note&&(_[e.id]=e.note)`. RELATIONSHIP_ADD and RELATIONSHIP_UPDATE carry it too:
  `...originApplicationId:e.origin_application_id,note:e.note`.
- The pending-incoming row hides the note behind an eye-icon button `sB0q4C` "View Request" /
  "Ver solicitação". Clicking it reveals the text and tracks
  `friend_request_note_viewed {location:"Friend Request Row", note_length}`.
- There is a privacy proto setting `hide_friend_request_notes` (field 33, user settings proto).

**Other `sendRequest` locations seen:** `"Add Friend"` (Friends tab), `"Group DM"`,
`"New User Onboarding"`.

---

## 2. Guild header rows in the channel sidebar

### Row list and order
In `35a17d1ed1df0fb9.js`, `rT` → `useGuildActionRows`:

```js
x&&E.push(GUILD_HUB_HEADER_OPTIONS), A&&f?E.push(GUILD_ONBOARDING_SETUP_PROGRESS):!g&&d&&m&&null!=c&&c.length>0?
E.push(GUILD_NEW_MEMBER_ACTIONS_PROGRESS_BAR):e.premiumProgressBarEnabled&&I>0&&E.push(GUILD_PREMIUM_PROGRESS_BAR),
!x&&d&&E.push(GUILD_HOME), C&&E.push(GUILD_SPACE), l&&E.push(GUILD_SCHEDULED_EVENTS), !x&&N&&E.push(CHANNELS_AND_ROLES), ...
```

The order is:
1. Hub options
2. **One** progress row: the onboarding-setup progress (admins), or else the new-member-actions
   progress bar, or else the **boost bar**
3. Server Guide
4. Guild Space
5. Events
6. Channels & Roles
7. Role subscriptions
8. Shop
9. Game shop
10. Member safety
11. Boosts
12. Official messages
13. Game servers
14. Conjure

### Banner
CDN builder `getGuildBannerURL` (`P`, web):

```js
let a=(0,d.kr)(360*(0,d.mZ)()),l=m?"webp":"jpg",o=r&&H(i)?z(m):l,...t=`https://${c}/banners/${n}/${i}.${o}`;
let u={size:a};return"jpg"===o&&(u.quality="lossless"),"webp"===o&&r&&H(i)&&(u.animated=!0),t+=`?${...}`
```

The URL is `https://cdn.discordapp.com/banners/{guild_id}/{banner_hash}.{webp|jpg}?size={pow2(360×DPR)}`:
- With `jpg`, add `&quality=lossless`.
- For an animated (`a_`) hash when allowed, add `&animated=true` (webp).

When it shows (`2eff00ecd5f1e706.js`):

```js
getGuildBannerHash(){let{guild:e,hasChannelNotice:t}=this.props;return null==e||t?null:e.banner}
```

- It shows whenever `guild.banner != null` and no channel notice is shown. Then the list gets
  `paddingTop: 84` (`null!=t&&(o=84)`).
- The banner fades and translates with scroll (`pinBannerOrGuildInfo` → spring).
- **Animation** (`84a86368febb0393.js`) needs the guild feature `ANIMATED_BANNER`, an animated
  hash, the "animate" setting and no `disableBannerAnimation`. Then the banner plays for **5 s**
  once visible (`setTimeout(...,5e3)`) and again on hover.
- **Data:** `banner` is in GUILD_CREATE `properties`. No fetch.

### Boost goal bar (`GUILD_PREMIUM_PROGRESS_BAR`, now `GuildPowerupsProgressBarUI`)
- **Shown when** `guild.premiumProgressBarEnabled && maxBoostCount > 0`, and neither the
  onboarding-setup row nor the new-member-actions row takes the slot.
- **Data** (both in GUILD_CREATE `properties`, no fetch):
  - `premium_progress_bar_enabled`
  - `premium_subscription_count` (as `premiumSubscriberCount`)
- **Applied count:** the `GuildBoostingProgressBarPersistedStore` keeps the last seen count per
  guild, only to animate the fill from the old to the new value.
- **Max (goal)** is `ti(guild)`:
  - `M2T[TIER_3]` = **14** (`rE=Object.freeze({0:0,1:2,2:7,3:14})`). It is 0 when the guild has
    `PREMIUM_TIER_3_OVERRIDE`.
  - Plus the `boostPrice` of every power-up without `includedInLevel` whose `isEnabled` passes
    (module `568065`):
    - `GUILD_TAGS` 3
    - `ENHANCED_ROLE_COLORS` 3
    - `GUILD_TAGS_BADGE_PACK_PETS` 3
    - `..._FLEX` 5
    - `..._PLANT` 3
    - `..._CREEPY_CRAWLIES` 2
    - `GUILD_THEME` 3 (behind two experiments)
    - `GAME_SERVERS` 3 (behind an experiment)
  - `VANITY_URL` (5) is skipped because it is included in TIER_3.
  - **Inferred:** without the experiment-gated entries, the max is 14 + 19 = **33**. With
    them it is up to 39.
- **UI:**
  - The fill is `min(applied/max·100, 100)%`.
  - Left label: `NI6Ihe` "Boost Goal" / "Objetivo de impulso", plus 🎉 when the count is ≥ max.
  - Right label: `/rbPDs` "{applied}/{max} Boosts" / "{applied}/{max} Impulsos", or `Ehpq+7`
    "{applied} Boosts" / "{applied} Impulsos" once full. A chevron follows it.
  - Clicking opens the boosting page (`GUILD_BOOSTING_SIDEBAR_DISPLAY`).

### Server Guide (`GUILD_HOME`, row text `VbpLyU` "Server Guide" / "Guia do servidor")
The gate is `tD.d` (module `473529`) plus `!HUB`:
- The guild needs `COMMUNITY`, it is not `@me`/favorites, and the overlay is off.
- The normal path (`707167`) needs features `COMMUNITY` + `GUILD_SERVER_GUIDE` + `GUILD_ONBOARDING`.
- And then **either** (`978165`) the member is "new": `member.joined_at` is less than one week
  ago and member `flags` lacks `COMPLETED_HOME_ACTIONS`. **Or** the guild has a channel with the
  channel flag `IS_GUILD_RESOURCE_CHANNEL`.

Data:
- The row needs no fetch (guild features, the self member's `joined_at`/`flags`, channel flags,
  all from GUILD_CREATE).
- The page content is `GET GUILD_HOME_SETTINGS` = `/guilds/{id}/new-member-welcome` →
  `GUILD_HOME_SETTINGS_FETCH_SUCCESS` (`welcomeMessage`, `newMemberActions`,
  `resourceChannels`, `enabled`). It is fetched **lazily**: when the guide page mounts with no
  settings yet (`g===W.A?(0,F.ag)(n.id)`), or by the new-member-actions manager.
- `GET GUILD_MEMBER_ACTIONS` gives the per-member completed actions.

Clicking it goes to `/channels/{guild}/@home`-like `GUILD_HOME` static route (inferred name).

### Events (`GUILD_SCHEDULED_EVENTS`)
- **Shown when** `!HUB && (canCreateEvents || upcomingEvents.length > 0)` (`770666` +
  `508654`):
  - `canCreateEvents`: `ADMINISTRATOR` or `CREATE_EVENTS` on the guild, or the event permission
    on any channel.
  - `upcomingEvents`: index `GUILD_EVENT_UPCOMING(guildId)`, filtered to events with no channel
    or a channel the user can `VIEW_CHANNEL`.
- **Text:** `IBdqSu` "{n} Event(s)" / "{n} Evento(s)" when n>0, else `tlopTM` "Events" / "Eventos".
- It has an unread dot and a mention count (read state type `GUILD_EVENT`).
- Clicking opens the events modal.
- **Data:** in READY `guilds[].guild_scheduled_events` and GUILD_CREATE `guild_scheduled_events`
  (`t.guild_scheduled_events.forEach(e=>T(e))`), plus `GUILD_SCHEDULED_EVENT_*` dispatches.
  **No fetch is needed for the row.**
- `GET GUILD_EVENTS_FOR_GUILD` `/guilds/{id}/scheduled-events` exists, with no query in this build.
  `with_user_count` is **not found** in this bundle.
- Interested counts come from `GET GUILD_EVENT_USER_COUNTS`
  `/guilds/{id}/scheduled-events/{event}/users/counts?guild_scheduled_event_exception_ids=…`.
  The response has `guild_scheduled_event_count` and `guild_scheduled_event_exception_counts`.
  It is cached 30 min per event (`18e5`).

### Channels & Roles (`CHANNELS_AND_ROLES`)
- **Shown when** `!HUB && guild.features.has("COMMUNITY")`.
- **Text** (`202776`):
  - `h9mGOP` "Channels & Roles" / "Canais & Cargos" when the guild has
    `GUILD_ONBOARDING_HAS_PROMPTS`. That row goes to `CUSTOMIZE_COMMUNITY`.
  - Otherwise `et6wav` "Browse Channels" / "Conferir Canais", which goes to `CHANNEL_BROWSER`.
- A "new" badge shows for new channels or new onboarding questions.
- **Data:** guild features only, for the row.
- Onboarding prompts come from `GET GUILD_ONBOARDING` `/guilds/{id}/onboarding` →
  `GUILD_ONBOARDING_PROMPTS_FETCH_SUCCESS`. The exact trigger was not traced; **inferred** to be
  fetched when the Customize page opens.

---

## 3. Friends page "Active Now" column
**There is no dedicated REST route; it is computed on the client.** Store `NowPlayingViewStore`
(module `943577`) builds "parties":

```js
function $(){let e=P.A.getFriendIDs();return new Set(y.A.hasConsented(PERSONALIZATION)?[...m.A.getUserAffinities()
.filter(e=>e.communicationRank<=15).map(e=>e.otherUserId),...e]:e)}
function z(e){return M.A.findActivity(e,e=>e.type!==x.$pd.CUSTOM_STATUS)}
```

- **Users:** friends, plus (only with the PERSONALIZATION consent) users whose
  `communicationRank ≤ 15`.
- **Inputs:**
  - Presences: activities other than custom status, grouped by `party.id`, application, URL or
    name.
  - Voice states in channels the user can `VIEW_CHANNEL`.
  - Go-live streams.
- **Shown:** a party is shown if it has ≥1 voice channel, activity or stream, and is not a
  non-discoverable guild-only context.
- **Affinities route:** `GET USER_AFFINITIES_V2` `/users/@me/affinities/v2/users`.
  - Response: `user_affinities[]` with `other_user_id`, `user_segment`, `other_user_segment`,
    `communicationRank`/`communicationProbability`, `dmProbability`, `vcProbability`.
  - Fetched only with consent, refetched after 8 h (`UserAffinitiesV2Store`), throttled
    5 min when the column mounts (`Es=throttle(()=>uO.u(),3e5)`).
- Unknown application ids → `GET APPLICATIONS_PUBLIC` `/applications/public?application_ids=…`
  (repeated param).
- **Lifecycle:** `NOW_PLAYING_MOUNTED`/`UNMOUNTED` and recompute every 1 s throttle.
- **Empty state:** `ngJ/5u` and `99ZWxQ`. The heading is `F4Zggw` "Active Now".
- A consent card `NOW_PLAYING_CONSENT_CARD` shows if there is no personalization consent.
- Quests can be attached to a party (ignore).
- **Content inventory is not used here.**

---

## 4. Member list "Activity" section (cards such as "Top Artist — 18 tracks this week")

**Source: the content inventory global feed** (not presences).

**Route** `MY_CONTENT_INVENTORY` (module `86070`):

```js
MY_CONTENT_INVENTORY:e=>`/content-inventory/users/@me${null!=e?`?refresh_token=${e}`:""}`
i.Bo.get({url:l.Rsh.MY_CONTENT_INVENTORY(t),query:{for_game_profile:n===s.X1.GAME_PROFILE_FEED,feature:r}})
```

The request is `GET /content-inventory/users/@me?refresh_token={prev}&for_game_profile=false&feature=inbox`:
- `refresh_token` is left out on the first call.
- `feature` is one of `inbox|game_profile|recent_activity|overlay_invites|admin`. The global
  feed uses `inbox`.

Response fields used:
- `entries[].content`
- `unranked_game_entries[]`
- `refresh_token`
- `wait_ms_until_next_fetch`, converted to `expired_at`
- `refresh_stale_inbox_after_ms`
- `request_id`

**Manager** `ContentInventoryManager` (`819235`):
- **Triggers:** `POST_CONNECTION_OPEN`, `WINDOW_FOCUS`, `IDLE`, manual refresh, and the
  **gateway dispatch `CONTENT_INVENTORY_INBOX_STALE` `{refresh_after_ms}`**.
- **It skips the fetch when:**
  - the experiment is off,
  - the feed is hidden and already loaded,
  - the window is not focused,
  - the gateway is not connected,
  - the user has been idle for more than 15 min.
- **On failure:** retries with backoff (1, 2, 4, 8 min plus jitter), then the feed is cleared.

**Entry fields used:**
- `id`, `author_id`, `participants[]`
- `content_type`: 1 PLAYED_GAME, 2 WATCHED_MEDIA, 3 TOP_GAME, 4 LISTENED_MEDIA,
  5 LISTENED_SESSION, 6 TOP_ARTIST, 7 CUSTOM_STATUS, 8 LAUNCHED_ACTIVITY, 9 LEADERBOARD
- `traits[]` `{type, …}`, for example type 12 AGGREGATE_COUNT → `.count`, 2 DURATION_SECONDS,
  3 IS_LIVE `.is_live`, 1 FIRST_TIME, 8 STREAK_DAYS, 9 TRENDING_CONTENT

The "N tracks this week" text is `HtifnG` "Top Artist — {count} tracks this week", fed by the
AGGREGATE_COUNT trait.

**Member list section** (`f1f07381693dcba1.js`):
- Section title `6gwSFY` "Activity" / "Atividade". It shows 3 cards and can expand to all.
- Expanding tracks `memberlist_content_feed_toggled`.
- **Entry filter:** types PLAYED_GAME, WATCHED_MEDIA, TOP_GAME, TOP_ARTIST, LISTENED_SESSION,
  LAUNCHED_ACTIVITY.
- **Author filter:** authors who are members of this guild and can `VIEW_CHANNEL` the current
  channel. The client calls `requestMember` (op 8) for unknown authors.
- Entries are also dropped for authors whose DM is muted, and when impression-capped.
- **Gating:**
  - User experiment `"2026-03-content-inventory-memberlist-and-ranker"` (default
    `{enabled:true, impressionCappingEnabled:true}`; variation 0 turns it off).
  - Guild check `376261`: if the guild has `ACTIVITY_FEED_ENABLED_BY_USER` or
    `ACTIVITY_FEED_DISABLED_BY_USER`, that feature decides. Otherwise the section shows only when
    `member_count < 10000`.
  - Not in forum channels.
  - The user can hide the section (`CONTENT_INVENTORY_TOGGLE_FEED_HIDDEN` → a `HIDDEN_CONTENT_INVENTORY` row).
- **Not in READY/GUILD_CREATE.**

---

## 5. Bell button in the channel header
Component `187360` (`26245004f64583ff.js`):
- Icon `BellIcon`, or `BellSlashIcon` when the channel is muted or the resolved level ≠ ALL_MESSAGES.
- Tooltip `h850Ss` "Notification Settings" / "Config. de notificação".
- **Shift+click** toggles the channel mute directly
  (`updateChannelOverrideSettings({muted:!j})`).
- A normal click opens a popout (position bottom, align right). That popout is the menu
  `461678` with `navId:"channel-context"` and aria-label `Xm41aV` "Channel Actions" /
  "Ações de canal". It has three groups:
  1. Mute item (`995102`: "Mute Channel" with a duration submenu).
  2. Guild mute (only with `includeGuildMute`).
  3. Notification-level radios (`288104`):
     - "Use Category Default" (`wlrV1c`) or "Use Server Default" (`1Wn2M4`)
     - "All Messages" (`n/bTaY`)
     - an optional "all unreads + mentions" variant
     - "Only @mentions" (`L2hmYy`)
     - "Nothing" (`CtVGyQ`)
     - Voice channels get no radios. Stage channels get only default, mentions and nothing.
- Threads use the "thread-context" variant (`1NBjqb` "Thread Actions"), opened through
  `OPEN_THREAD_NOTIFICATION_SETTINGS`.
- Writes go to the user guild settings channel overrides (already done in Emerald).

---

## 6. What Emerald sends today vs the web client
`emerald-discord/src/rest/users.rs:53` and `packets/user.rs:8`:
- `POST /users/@me/relationships`
- body `{"username":…, "discriminator":null}`
- `X-Context-Properties` `{"location":"Add Friend"}`
- with captcha

The UI trims and strips a leading `@`.

**This matches the web client exactly for the no-note case.** The web only adds:
- `"note": "<≤120 chars, newlines→spaces, trimmed>"` when non-empty, and only with the
  `2026-03-friend-request-message` experiment on;
- captcha fields spread into the body on retry, which Emerald already does through `with_captcha!`.

To add notes, Emerald needs:
- `friend_request(username, note: Option<&str>)`, which leaves out `note` when it is None or empty;
- the `note: Option<String>` field on `model::Relationship` (it is in READY and in
  RELATIONSHIP_ADD/UPDATE).

Whether the account is in the experiment cannot be checked offline (**inferred:** the server
probably accepts `note` regardless; unverified).
