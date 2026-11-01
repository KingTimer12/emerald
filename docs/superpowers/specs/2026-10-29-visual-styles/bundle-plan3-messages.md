# Bundle findings for plan 3, part A (saved messages, forward, hover bar, timestamps)

Source: the web client's chunks in `target/discord-intl/` (read-only grep, no network).
Main chunk: `web.057e7776e1c6251b.js` (below "web"). Other chunks are named by their hash.
Locale chunks used to resolve message keys: `26d528cd23025094.js` (en-US),
`53088f6d9d7a0c0a.js` (pt-BR). "Inferred" marks anything not read directly from code.

---

## 1. Saved messages: reminders and bookmarks

One server object serves both. A **reminder** is a saved message with `due_at`. A **bookmark**
is one without `due_at`. There is no separate flag.

### Routes (web, module `216623`)

```js
GET_SAVED_MESSAGES:"/users/@me/saved-messages",GET_BOOKMARKS:"/users/@me/saved-messages/bookmarks",
PUT_SAVED_MESSAGE:(e,t)=>`/users/@me/saved-messages/${e}/${t}`,DELETE_SAVED_MESSAGE:(e,t)=>`/users/@me/saved-messages/${e}/${t}`
async function o(e){let t=await i.Bo.put({url:l.Rsh.PUT_SAVED_MESSAGE(e.channelId,e.messageId),body:{due_at:e.dueAt,source:e.source},...});return(0,s.iz)(t.body)}
async function d(e){return await i.Bo.del({url:l.Rsh.DELETE_SAVED_MESSAGE(e.channelId,e.messageId),...}),!0}
e=await i.Bo.get({url:l.Rsh.GET_SAVED_MESSAGES,query:{bookmark_ids:!0},...})
... dispatch({type:"SAVED_MESSAGES_UPDATE",reminders:t.reminders.map(s.iz),bookmarkIds:t.bookmark_ids})
e=await i.Bo.get({url:l.Rsh.GET_BOOKMARKS,query:{before:n??void 0,limit:25},retries:2,...})
... nextBefore:d.results.at(-1)?.save_data.saved_at??n,bookmarks:d.results.map(s.iz),hasMore:d.has_more
```

- **`GET /users/@me/saved-messages?bookmark_ids=true`** returns `{ reminders: [SavedMessage], bookmark_ids: [message_id] }`: every reminder in full, and bookmarks only as message ids. It runs on `POST_CONNECTION_OPEN` (SavedMessagesManager, module `199543`), only when the store is stale. If it fails, the lists are emptied.
- **`GET /users/@me/saved-messages/bookmarks?before=<saved_at>&limit=25`** returns `{ results: [SavedMessage], has_more }`.
  - The cursor `before` is the **`saved_at` string of the last result**, not a snowflake. It is left out on the first page.
  - More loads within 250 px of the bottom, or with the button `XBlaiC` "Load more messages" / "Carregar mais mensagens".
- **`PUT /users/@me/saved-messages/{channel_id}/{message_id}`** with body `{ due_at, source }`, answered by one SavedMessage.
  - `due_at` is a JS `Date`, so on the wire it is ISO-8601 with milliseconds and `Z`, e.g. `"2026-10-30T12:00:00.000Z"` (Inferred from JSON serialization).
  - A bookmark sends `dueAt: void 0`, so `due_at` is **left out**.
  - Editing or snoozing a reminder is the same PUT with a new `due_at`.
  - The client never sends `notes`, although the response has it.
  - `source` (module `738125`) is one of `"context_menu"`, `"long_press_action_sheet"`, `"message_toolbar"`, `"for_later_list"` or `"notifications_inbox"`.
- **`DELETE /users/@me/saved-messages/{channel_id}/{message_id}`**, with no body, removes a bookmark or a reminder. It is also what "Mark Complete" does.

### SavedMessage shape (parser `iz`, module `756377`)

```js
{message:null!=e.message?(0,a.rh)(e.message):null,saveData:{channelId:(t=e.save_data).channel_id,messageId:t.message_id,
savedAt:new Date(t.saved_at),authorSummary:t.author_summary,channelSummary:t.channel_summary,messageSummary:t.message_summary,
guildId:0===t.guild_id?void 0:t.guild_id,authorId:0===t.author_id?void 0:t.author_id,notes:t.notes,
dueAt:null!=t.due_at?new Date(t.due_at):void 0}}
```

- `message` is a full message object, or `null` when the message cannot be read. The list then shows `o572Fe` "A message you bookmarked could not be found." / `wuQm+j` "A message you set a reminder for could not be found.", with only a remove button.
- `guild_id` and `author_id` come back as the **number `0`** when absent, so treat `0` as none.

### Limits and errors (chunk `42eb32ee6e2288e4.js`, module `568467`)

```js
if(t?.body?.code===u.t02.TOO_MANY_SAVED_MESSAGES){let t=null!=e.dueAt;return l.A.show({title:d.intl.string(d.t.mlbiZW),
body:d.intl.formatToPlainString(t?d.t.Anr1Dg:d.t["1zVbEG"],{max:t?50:500}),cancelText:...BddRzS,confirmText:...ZGbTcy,
onConfirm:()=>s._.dispatch(u.jej.TOGGLE_INBOX,t?i.Y2.REMINDERS:i.Y2.BOOKMARKS)})
```

- Error code `TOO_MANY_SAVED_MESSAGES = 30074`.
- Limits: **50 reminders** and **500 bookmarks**, hard-coded in the client.
- The limit dialog uses `mlbiZW` "Oops! Limit reached." / "Opa! Limite alcançado." and the body `Anr1Dg` / `1zVbEG` "You've reached the limit for reminders/bookmarks ({max} total)...". Its buttons are "Okay" and `ZGbTcy` "Manage Messages" / "Gerenciar mensagens"; the second opens the Inbox on the matching tab.
- Any other error shows a red toast with `body.message`, or with `R0RpRX` "Oops, something went wrong..." / "Ops, algo deu errado...".
- Success toasts, with a `clock` or `bookmark` icon:
  - `i1IsOy` "Reminder added." / "Lembrete adicionado."
  - `WQdL/6` "Bookmark added. For your eyes only." / "Adicionada aos favoritos. Só para seus olhos."
  - `D0tS02` "Reminder removed." / "Lembrete removido."
  - `5KOMiV` "Bookmark removed." / "Removida dos favoritos."

### Gateway (web, the dispatch table)

```js
eB(["SAVED_MESSAGE_CREATE"],e=>{ej({type:"SAVED_MESSAGE_CREATE",savedMessage:(0,er.iz)(e)})}),
eB(["SAVED_MESSAGE_DELETE"],e=>{ej({type:"SAVED_MESSAGE_DELETE",savedMessageData:(0,er.x6)(e)})})
function d(e){return{channelId:e.channel_id,messageId:e.message_id}}   // x6
```

- **`SAVED_MESSAGE_CREATE`** carries a full SavedMessage (`{message, save_data}`). The store upserts it, so a PUT that updates an existing save probably also comes back this way (Inferred).
- **`SAVED_MESSAGE_DELETE`** carries `{ channel_id, message_id }`.
- **Store (`SavedMessagesStore`, module `85109`):**
  - Entries are keyed by `${channelId}-${messageId}`.
  - Reminders are sorted by `dueAt`, ascending. Bookmarks are sorted by `savedAt`, newest first (`1e13 - savedAt`).
  - `MESSAGE_UPDATE` patches the cached message. `MESSAGE_DELETE` sets the entry's `message` to null and keeps the entry.
  - Channel, guild or role changes mark the store stale, which leads to a refetch.
- **Reminder notification (module `343509`):**
  - A `setTimeout` is armed for the next future reminder, only when it is due within a week.
  - When it fires, the client dispatches `MESSAGE_REMINDER_DUE` and shows a desktop notification titled `IjZJB5` "⏰ Reminder Due" / "⏰ Lembrete chegando", with `notif_type:"MESSAGE_REMINDER_DUE"`.
  - Clicking the notification opens the message's channel link (`/channels/…/…/…`).
  - `getOverdueMessageReminderCount()` counts the reminders whose `dueAt` has passed.

**Experiment gate:** none found. The context menu hides the items only for a guild channel
without `READ_MESSAGE_HISTORY` when nothing is saved yet.

### Message context menu (chunk `8b980d157af44f08.js`, module `880457`)

- **`id:"bookmark"`**, hidden when the message has a reminder.
  - Label `tpxJto` "Bookmark Message" / "Favoritar mensagem", or `SvXS1Z` "Remove Bookmark" / "Remover marcação" when it is already bookmarked.
  - Sends `source:"context_menu"`.
- **`id:"reminder"`**, a submenu labeled `mJ3P0N` "Create Reminder" / "Criar lembrete", or `vrbqs1` "Edit Reminder" / "Editar lembrete" when a reminder exists.
  - Without a reminder: one group titled `roMu1H` "Remind Me" / "Lembrar-me", holding the time options.
  - With a reminder: a group titled with the long due text, holding `mark-complete` `yjGtdJ` "Mark Complete" / "Marcar como concluído" (a **DELETE**). Then a group titled "Edit Reminder" holding the time options again.

**Time options** (chunk `b20726321bcd762e.js`, module `195404`; local time, computed with moment):

| # | Due time | en-US | pt-BR |
|---|---|---|---|
| 1 | now + 30 min | `OV8l/H` "in 30m" | "em 30min" |
| 2 | now + 1 h | `zf0R+0` "in 1h" | "em 1h" |
| 3 | now + 4 h | `5gztZN` "in 4h" | "em 4h" |
| 4 | today 09:00 if before 9h, else tomorrow 09:00 | `FnFI3m` "Today at 9:00am" / `7MKr2P` "Tomorrow at 9:00am" | "Hoje às 9h" / "Amanhã às 9:00" |
| 5 | Monday 09:00 (see note) | `q+Ls05` "Monday at 9:00am" | "Segunda-feira às 9:00" |
| 6 | opens a modal | `OLA8Zi` "Custom..." | "Personalizado" |

```js
{getDueAt:()=>{let e,t=g()().day();return e=0===t||1===t&&g()().startOf("day").add(9,"hours").isAfter(g()())?1:8,
 g()().day(e).startOf("day").add(9,"hours").toDate()},getLabel:()=>_.intl.string(_.t["q+Ls05"])}
```

- Option 5 picks this week's Monday on Sunday, or on Monday before 9h, and next week's Monday otherwise.
- **Custom modal** (chunk `60aaf841a9ea1750.js`):
  - title `VKsXpY` "Set Custom Reminder" / "Definir lembrete personalizado";
  - date picker `pSZKvM` "Date" / "Data" (`minDate` is today) and time picker `GOmEb8` "Time" / "Hora";
  - buttons `R3BPH+` "Save" / "Salvar" and `ETE/oC` "Cancel" / "Cancelar".

**Due text** (module `704456`, moment `humanize()`, refreshed every minute):

| Style | Due in the future | Overdue |
|---|---|---|
| LONG (menus) | `TjNWNF` "Reminder due in {duration}" / "Lembrete previsto em {duration}" | `haia16` "Reminder due {duration} ago" / "Lembrete expirado {duration} atrás" |
| SHORT (list chips) | `H4gnX9` "Due in {duration}" / "Previsto em {duration}" | `Uq7Y+7` "Due {duration} ago" / "Ativado {duration} atrás" |

### Hover bar

- The bookmark button lives only in the **Shift-expanded** bar (section 3).
- Its label is `9p3D9p` "Bookmark Message", or `LHUP9D` "Remove Bookmark" when already bookmarked.
- It sends `source:"message_toolbar"` and is hidden when the message has a reminder.
- There is **no** reminder button in the hover bar.

### Inbox tabs (chunk `92a208c82c792a3e.js`)

**Tab order:**
1. `sRUdB8` "Unreads" / "Não lidas"
2. `GRZF96` "Mentions" / "Menções"
3. **`2pAkDA` "Bookmarks" / "Favoritos"**
4. **`aUXxzT` "Reminders" / "Lembretes"**. When there are overdue reminders, it becomes `5en8ya` "Reminders ({count})" / "Lembretes ({count})", where `count` is the overdue count.
5. `Fn6Odn` "Scheduled" / "Agendado", only when scheduled messages are enabled.

**The list:**
- It is one flat list, with **no sections**. Overdue reminders come first because the list is sorted by `dueAt`.
- Each row has a channel header and the message rendered compact or cozy.
- For reminders, the header has a status chip: a clock and the SHORT text, or an error icon when the reminder is overdue.
- Hover actions on a row:
  - reminders only: `vrbqs1` "Edit Reminder" with a pencil. When overdue, it becomes `GtBCnz` "Snooze Reminder" / "Lembrete de soneca", which opens the same time menu;
  - `+TSRGD` "Jump to Message" / "Ir para a mensagem";
  - `a6gcZ/` "Remove Reminder" / "Remover lembrete", or `SvXS1Z` "Remove Bookmark". This is a DELETE.
- Clicking a row jumps to the message and closes the popout; Shift keeps the popout open.

**Empty state:** an image, a heading, a body, and three fake "Wumpus" sample messages with a
highlighted pill.
- Reminders heading: `5Iw19e` "Stay organized, never forget a follow-up" / "Mantenha as coisas ordem, sem deixar nada passar".
- Bookmarks heading: `93WOd1` "Collect your favorite memories" / "Guarde suas memórias favoritas".
- Reminders body: `E+qms7` "Right-click any message and pick **{itemName}**. We'll nudge you here when it's due." / "Clique com o botão direito em qualquer mensagem e selecione **{itemName}**. Vamos te avisar aqui quando chegar a hora."
- Bookmarks body: `legvTg` "... Find it here, for your eyes only." / "... Encontre-a aqui, só para você."
- `itemName` is "Create Reminder" or "Bookmark Message".

**Unused strings.** `0KeMA4` "All Reminders" / "Todos os lembretes", `j5ZHEO` "Overdue
Reminders" / "Lembretes atrasados", `nSnUfY` and `L+U0JU` exist in the locale files, but **no
downloaded code chunk references them**. They may belong to mobile. Do not build
overdue/upcoming sections from them.

**Analytics:**
- impression `FOR_LATER_LIST_VIEWED {tab_type, total_count, overdue_count}`;
- `trackJump(channelId, messageId, "Bookmarks"|"Reminders")`;
- location `FOR_LATER_POPOUT`.

---

## 2. Forward message (chunk `90379e04e34f83f9.js`)

```js
let r={guild_id:i,channel_id:e.channel_id,message_id:e.id,type:N.S.FORWARD,forward_only:t?.onlyAttachmentIds!=null||t?.onlyEmbedIndices!=null?
 {attachment_ids:t.onlyAttachmentIds,embed_indices:t.onlyEmbedIndices}:void 0},o=0,u=t?.withMessage;
if(null!=u){let[e,n]=(0,V.Ay)(u);e&&(u=n,o=(0,F.UI)(o,U.pr7.SUPPRESS_NOTIFICATIONS))}
await D.A.sendMessage(l.id,s /* parse(l,"") */,!1,{messageReference:r,location:B.Hx.FORWARDING,eagerDispatch:!1,flags:o}),
null==u||""===u||(0,G.lP)(l,L.A)||await D.A.sendMessage(l.id,O.Ay.parse(l,u),!1,{location:B.Hx.FORWARDING,flags:o})
sendForwards:(e,n,t)=>M()(n.map(n=>H.sendForward(e,n,t)))   // allSettled-style, one per destination
```

### What is sent

**Message 1:** `POST /channels/{dest_id}/messages` with the normal send body:
- `mobile_network_type`, `content:""`, `nonce`, `tts:false`, `flags`;
- `message_reference: { guild_id, channel_id, message_id, type: 1, forward_only? }`.
  - `type` is `FORWARD = 1` (module `762230`).
  - `guild_id` is the **source** guild. It is `undefined`, so left out, for a DM source.
  - `forward_only: {attachment_ids, embed_indices}` is present only when forwarding a subset.

**Message 2**, only when the optional text is not empty: a normal message with that text and
no reference, sent **after** message 1 succeeds.
- It is skipped for slowmode destinations (`G.lP`), as string `xJFpij` says: "The message you added will not be sent to slowmode channel(s)".
- When the text starts with `@silent`, both messages get `SUPPRESS_NOTIFICATIONS` (4096).

**Location.** `location:"forwarding"` (`Hx.FORWARDING`) is sent as `context: {location}`, which
means the `X-Context-Properties` header. It is not in the body:

```js
l.Bo.post({url:A.Rsh.MESSAGES(n),body:o,context:null!=a?{location:a}:void 0,...})
```

**Files and embeds** are not re-uploaded. The client sends only the reference, and the server
builds `message_snapshots` (Inferred from the empty content and the preview reading
`messageSnapshots`).

**Caution.** A destination that is a user with no DM channel is first resolved with
`getOrEnsurePrivateChannel`, which **opens a DM** (`POST /users/@me/channels`).

### The modal

- **Header:** title `+SkRRj` "Forward To" / "Encaminhar para". Subtitle `VA+btJ` "Select where you want to share this message." / "Selecione onde você quer compartilhar esta mensagem."
- **At most 5 destinations** (`eh=ed>=5`). At 5, the subtitle becomes `3Fbkir` "Maximum {count} places at once." / "Máximo de {count} lugares simultaneamente.", and the other rows are disabled.
- **Search box:** `5h0QOP` "Search" / "Buscar", autofocused. With no results it shows `V6nAfF` "We searched far and wide. Unfortunately, no results were found." / "Buscamos por toda parte. Infelizmente, não encontramos nada."
- **List with no query** (module `446244`):
  - order: the selected destinations, then channel history (recently **visited**), then frequent channels (frecency);
  - duplicates are removed;
  - only sendable rows are kept: DMs, group DMs, and guild channels with `VIEW_CHANNEL` and `SEND_MESSAGES`, but not forum or media channels;
  - the source channel is excluded unless selected;
  - the list is cut to **15**.
- **With a query:** the quick-switcher search with frecency boost, excluding yourself.
- **Toggling a row** puts it at the front of the selection. Screen readers hear `u2qHzE` / `+yF0ds` "Added/Removed {name}. N recipients selected".
- **Preview:** the content, or a file summary (`h4pFfU` "{n} images", `SJ6pPX` "{n} videos", `Lr0Top` images + videos, `89ihS8` "{n} files"), with a 56 px thumbnail and "+N" for several files.
- **Optional text:** a real chat input (`FORWARD_MESSAGE_INPUT`) with autocomplete and emoji. Its draft is saved per source channel. Placeholder `ZroO3G` "Add an optional message..." / "Adicione uma mensagem opcional...".
- **Send button:**
  - `TXNS7S` "Send" / "Enviar" for one destination or fewer, `jWtYUm` "Send ({count})" / "Enviar ({count})" for more;
  - the send icon is at the end;
  - disabled with 0 destinations or when the text is too long;
  - aria description `5X5Oe2` "Send to {names}".
- **After sending:**
  - success shows the toast `kwmYkt` "Message forwarded!" / "Mensagem encaminhada!", throttled to one every 3 s;
  - with one destination, the app navigates there;
  - failures open a retry flow for the failed destinations (`vK`).
- **Opening and analytics:**
  - from the hover bar or context menu, with `source:"message-actions"`;
  - with the **F** key on a focused message, with `source:"keyboard-shortcut"`;
  - opening tracks `pp(channel_id, message_id, source)`;
  - sending tracks `TA({hasError, hasContextMessage, numDestinations, numDestinationChanges, numQueryChanges, anyDestinationHasSlowmode})`.
- **Label** (hover bar and context menu): `I3ltXO` "Forward" / "Encaminhar".

---

## 3. Message hover bar (chunk `7174e79594908897.js`, function `nm`)

**When it expands.** The bar expands while **Shift is held** and no popout is open. Module
`311283` tracks `e.shiftKey` on keydown, keyup and mousemove.

```js
isExpanded:B&&!K&&!l&&!i&&!s
// B = useShiftKey(window), K = keyboard mode, l/i/s = emoji / burst-emoji / "More" popouts open
```

**Button order, left to right, as rendered:**

1. *Expanded only*, in this order:
   - `copy-id` `zBoHlf` "Copy Message ID" / "Copiar ID da mensagem". Needs Developer Mode. Shift+click copies `${channel_id}-${message_id}`.
   - `copy-link` `WqhZss` "Copy Link" / "Copiar link".
   - `configure`, for join messages.
   - `mark-unread` `RpE9k7` "Mark Unread" / "Marcar como não lido".
   - `bookmark` `9p3D9p` / `LHUP9D`, hidden when the message has a reminder.
   - `pin` `CvQ18w` "Pin Message" / "Fixar mensagem", or `Bse+F/` "Unpin Message" / "Desafixar mensagem". Needs pin permission.
   - `thread` `rBIGBL` "Create Thread" / "Criar tópico", only when Forward is also available.
   - `reply-self` `5IEsGx` "Reply" / "Responder", for **your own** message.
2. *Normal bar only:* up to **3 quick-reaction emoji**, then a separator. They are the guild's top emoji, padded with 💯 😆 💖.
3. `add-reaction` `lfIHs4` "Add Reaction" / "Adicionar reação".
4. `reply-other` "Reply", for **others'** messages.
5. `edit` `bt75uw` "Edit" / "Editar", for your own message.
6. `forward` `I3ltXO` "Forward".
7. `thread` "Create Thread" when Forward is unavailable, or `view-thread` `39d0Wj` "View Thread" / "Ver tópico".
8. `guild-official`, and `publish` `MFGE51` "Publish" (announcement channels).
9. *Expanded only:* `delete` `oyYWHE` "Delete" / "Excluir", shown as dangerous, in place of "More".
10. `more` `UKOtz+` "More" / "Mais". It opens the full context menu (`navId:"message-actions"`) and is hidden when the bar is expanded and you can delete.

Shift+click on Delete or Pin skips the confirmation (module `519222`).

**Copy link format.**

```js
`${location.protocol}//${location.host}${CHANNEL(guild_id, channel.id, message.id)}`
CHANNEL:(e,t,n)=>{let i=null==t?`/channels/${e||s}`:`/channels/${e||s}/${t}`;return null==n?i:`${i}/${n}`}
```

- This gives `https://discord.com/channels/{guild_id|@me}/{channel_id}/{message_id}`. Inferred: the fallback `s` is `@me`.
- An optional `?summaryId=` suffix exists for summaries.
- The context-menu item (module `345254`) builds the same URL, labeled `Xrt5Po` "Copy Message Link" / "Copiar link da mensagem".
- Both track `MESSAGE_LINK_COPIED {message_id, channel}`.

**Context menu groups**, in order (module `355682`):
1. the emoji row;
2. reactions;
3. edit `fsBWmS` "Edit Message", reply, forward, thread;
4. copy-text `JrGD7E` "Copy Text", translate, **pin**, official, **bookmark + reminder**, apps, resend, **mark-unread**, configure, **copy-link**, view-conversation, tts, …;
5. remove reactions, delete `xwMqD7` "Delete Message", report (the slot of delete is Inferred);
6. Copy Message ID, or "Message details" with copy IDs and links.

---

## 4. Timestamps (module `58703` in web; the component is in chunk `bd5374d481b1db6c.js`)

```js
T=a.useMemo(()=>null!=d?(0,o.i$)(k,d,F):s?(0,o.i$)(k,"LT",F):(0,o.mk)(k,!0),[k,d,s,F])   // s = compact
function C(e){let t,n=arguments[1]??!1,...,l=T(L(e),s.toDate());if(l<-1)return N(e,"L LT",i);if(l<0)t="lastDay";
 else if(l<1){if(n)return N(e,"LT",i);t="sameDay"}else t=l<2?"nextDay":"sameElse";return N(e,a.calendar(t,R(e),s),i)}   // mk
```

`T` is the difference in **local calendar days**.

**Cozy header** (`mk(date, true)`):

| When | Format | en-US | pt-BR |
|---|---|---|---|
| Today | `LT` | time only, e.g. "3:45 PM" (**no "Today at"**) | "15:45" |
| Yesterday | `calendar("lastDay")` | "Yesterday at 3:45 PM" | "Ontem às 15:45" |
| Older | `L LT` | "10/28/2026 3:45 PM" | "28/10/2026 15:45" |

- pt-BR comes from the moment locale in chunk `cb014f5c885ba184.js`: `LT:"HH:mm"`, `L:"DD/MM/YYYY"`, `lastDay:"[Ontem às] LT"`, `sameDay:"[Hoje às] LT"`.
- en-US comes from moment's built-in `en` (Inferred: `L` is "MM/DD/YYYY", `LT` is "h:mm A").
- The text goes through `i$`, which compiles moment tokens and applies the user's 12h/24h setting (`PZ`: AUTO, H12 or H23).

**Compact mode and the cozy gutter** both use `LT`. A cozy follow-up message renders
`{compact:!0, isInline:!1, isVisibleOnlyOnHover:!0, cozyAlt:!0}`, so the time only ("3:45 PM" or
"15:45") appears on hover.
- In compact mode the time is wrapped in aria-hidden `[` `]`. In cozy mode a ` — ` separator precedes it.
- Inferred: CSS hides the brackets in `cozyAlt`.

**The `<time dateTime=ISO>` element:**
- tooltip `LLLL`, e.g. "Thursday, October 30, 2026 3:45 PM", and in pt-BR "quinta-feira, 30 de outubro de 2026 às 15:45";
- aria-label `K7`, which is the calendar text, or `LLL` for older dates;
- for edited messages, `CDzOFd` "Edited {timeFormatted}" / "Editado {timeFormatted}".

---

## 5. Mark Unread (chunk `e29929127381ce99.js`, module `439147`)

```js
let d=o.A.getMessages(e),p=d.toArray().filter(e=>0>c.default.compare(e.id,r)).sort(...).reverse()[0],
f=null==p?c.default.atPreviousMillisecond(r):p.id,m=0;d.forAll(e=>{c.default.compare(e.id,f)>0&&(0,s.Wm)(e,a)&&m++});
... k.isThread()&&(k.isArchivedThread()&&await l.A.unarchiveThread(k,!1),i.A.hasJoined(e)||await l.A.joinThread(k,"Mark Unread")),
t.Bo.post({url:h.Rsh.MESSAGE_ACK(e,f),body:{manual:!0,mention_count:m},oldFormErrors:!0,rejectWithError:!0})
```

- **Route:** `POST /channels/{channel_id}/messages/{ack_id}/ack` with body `{ "manual": true, "mention_count": N }`. There is no `token` and no `last_viewed`.
- **`ack_id`** is the loaded message just **before** the chosen one. When none is loaded, it is the chosen id minus 1 ms as a snowflake (`atPreviousMillisecond`).
- **`mention_count`** counts the loaded messages after `ack_id` that mention you (`Wm`).
- **Threads:** the client first unarchives an archived thread, then joins it with location `"Mark Unread"` if you are not a member. Both are writes.
- **Triggers:**
  - the Shift-expanded hover bar;
  - the context menu;
  - Enter with a modifier on a focused message (`case"enter":s&&(0,tC.cl)(d,c)`). Inferred: `s` is Alt, which matches the web client's Alt+click.
- **Label:** `RpE9k7` "Mark Unread" / "Marcar como não lido".

---

## Not found

- **Experiment gating** saved messages: none in the paths read.
- **Unreferenced strings:** "All Reminders" and "Overdue Reminders" (`0KeMA4`, `j5ZHEO`), and the other "Reminders" keys (`nSnUfY`, `L+U0JU`). No downloaded code chunk uses them.
- **The exact en-US text for older dates** (`L LT` through `i$`) is Inferred from moment's defaults. I did not trace it through the Intl formatter compiler.
