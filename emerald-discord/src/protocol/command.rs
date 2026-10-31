//! Requests from the UI to the network thread, and the values they carry.

use std::sync::Arc;

use super::{
    ChannelEdit, OwnPresence, ProfileEdit, SearchQuery, SearchScope, SettingsChange, SettingsEdit, ThreadQuery, VoiceCall, VoiceControls,
};
use crate::model::{ChannelKind, Emoji, Gif, Snowflake, UserSettings};

/// Requests flowing from the UI to the network thread.
pub enum Command {
    History {
        channel: Snowflake,
        page: Page,
    },
    /// First messages of these posts of `forum`.
    ForumPosts {
        forum: Snowflake,
        threads: Vec<Snowflake>,
    },
    /// A page of `channel`'s threads matching `query`, from `offset`:
    /// archived ones when no name is given, else any whose name matches.
    ThreadSearch {
        channel: Snowflake,
        query: ThreadQuery,
        offset: usize,
    },
    /// Pinned messages of `channel`, pinned before `before` (the
    /// `pinned_at` of the last one shown) when paging.
    Pins {
        channel: Snowflake,
        before: Option<String>,
    },
    /// A page of messages matching `query`, from `offset`.
    Search {
        scope: SearchScope,
        query: SearchQuery,
        offset: usize,
    },
    /// Recent mentions of the user, older than `before` when paging.
    Mentions {
        before: Option<Snowflake>,
    },
    /// The first page of `channel` newer than `after`, the last message
    /// read there, or its newest page when none was read, for the Inbox's
    /// Unreads tab.
    Unread {
        channel: Snowflake,
        after: Option<Snowflake>,
    },
    /// Changes the notification settings of a server, or of the DMs.
    GuildSettings {
        guild: Option<Snowflake>,
        change: SettingsChange,
    },
    /// Marks these channels read, each up to its message, as "Mark As
    /// Read" on a server does.
    AckBulk {
        channels: Vec<(Snowflake, Snowflake)>,
    },
    Send {
        channel: Snowflake,
        content: String,
        nonce: String,
        reply: Option<ReplyTo>,
        files: Vec<Upload>,
    },
    /// A sticker sent on its own, as picking one in the web client does.
    SendSticker {
        channel: Snowflake,
        sticker: Snowflake,
        nonce: String,
    },
    /// A GIF from the picker: its Tenor page sent as the message, after
    /// telling Discord which one was picked for `query`.
    SendGif {
        channel: Snowflake,
        gif: Gif,
        query: String,
        nonce: String,
    },
    /// GIFs for the picker: a search, or the trending ones.
    Gifs {
        query: Option<String>,
    },
    /// Discord's standard sticker packs, asked for once a session.
    StickerPacks,
    /// The slash commands of a guild, a DM or the user's own apps.
    ListCommands(IndexTarget),
    /// Runs a slash command in `channel`; the app answers through the
    /// gateway like any message.
    Interact {
        application: Snowflake,
        guild: Option<Snowflake>,
        channel: Snowflake,
        data: serde_json::Value,
        nonce: String,
    },
    /// Starts a thread in `parent`, from `message` when given.
    CreateThread {
        parent: Snowflake,
        message: Option<Snowflake>,
        name: String,
        kind: ChannelKind,
        auto_archive: u32,
    },
    /// Starts a post in `forum`, its first message holding `content`.
    CreatePost {
        forum: Snowflake,
        name: String,
        tags: Vec<Snowflake>,
        content: String,
        auto_archive: u32,
        /// Uploaded to the forum first, then named in the post's message.
        files: Vec<Upload>,
    },
    ThreadMembership {
        thread: Snowflake,
        join: bool,
        /// Forum posts are joined from the post's own toolbar button,
        /// other threads from the header's overflow menu.
        forum_post: bool,
    },
    /// The gateway echoes the outcome as MESSAGE_UPDATE.
    EditMessage {
        channel: Snowflake,
        message: Snowflake,
        content: String,
    },
    /// The gateway echoes the outcome as MESSAGE_DELETE.
    DeleteMessage {
        channel: Snowflake,
        message: Snowflake,
    },
    /// Posts a poll; the gateway brings the message.
    SendPoll {
        channel: Snowflake,
        nonce: String,
        poll: NewPoll,
    },
    /// Our vote on a poll, all of it: the answers picked, or none to take
    /// it back. The gateway echoes each vote that changed.
    VotePoll {
        channel: Snowflake,
        message: Snowflake,
        answers: Vec<u32>,
    },
    /// Pins or unpins; the gateway echoes it as MESSAGE_UPDATE.
    PinMessage {
        channel: Snowflake,
        message: Snowflake,
        pin: bool,
    },
    /// Adds (`add`) or removes our normal reaction with `emoji`. The UI
    /// shows it at once; the gateway echo then changes nothing.
    React {
        channel: Snowflake,
        message: Snowflake,
        emoji: Emoji,
        add: bool,
        source: ReactionSource,
    },
    SendFriendRequest {
        username: String,
    },
    AcceptFriend {
        user: Snowflake,
    },
    /// Removes a friend, declines an incoming request or cancels an outgoing one.
    RemoveRelationship {
        user: Snowflake,
    },
    /// Blocks `user`, or unblocks them.
    Block {
        user: Snowflake,
        block: bool,
    },
    /// Ignores `user`, or stops.
    Ignore {
        user: Snowflake,
        ignore: bool,
    },
    /// A friend's nickname; `None` clears it.
    SetNickname {
        user: Snowflake,
        nickname: Option<String>,
    },
    /// The user's note about `user`, read when their profile opens.
    FetchNote {
        user: Snowflake,
    },
    /// Empty clears the note.
    SetNote {
        user: Snowflake,
        note: String,
    },
    /// The friends the user shares with `user`.
    MutualFriends {
        user: Snowflake,
    },
    /// Starts a group DM with these friends; it opens when made.
    CreateGroup {
        users: Vec<Snowflake>,
    },
    /// Adds friends to a group DM, or to a 1:1 DM, which makes a new group
    /// that then opens.
    AddRecipients {
        channel: Snowflake,
        users: Vec<Snowflake>,
    },
    /// Removes someone from a group DM the user owns.
    RemoveRecipient {
        channel: Snowflake,
        user: Snowflake,
    },
    /// Renames a group DM; `None` names it after its members again.
    RenameGroup {
        channel: Snowflake,
        name: Option<String>,
    },
    /// A group DM's icon: an image's media type and bytes; `None`
    /// removes it.
    SetGroupIcon {
        channel: Snowflake,
        icon: Option<(&'static str, Arc<[u8]>)>,
    },
    /// Closes a DM, or leaves a group DM.
    CloseChannel {
        channel: Snowflake,
    },
    /// Accepts a message request (or a DM marked spam), or ignores it.
    AnswerRequest {
        channel: Snowflake,
        accept: bool,
    },
    /// "Invite People" on `channel`, from the surface `location` names.
    CreateInvite {
        channel: Snowflake,
        location: &'static str,
    },
    /// Resolves an invite for its card under a message.
    ResolveInvite {
        code: String,
    },
    /// "Leave Server".
    LeaveGuild {
        guild: Snowflake,
    },
    /// Deletes several messages of `channel` at once; the gateway brings
    /// MESSAGE_DELETE_BULK.
    BulkDelete {
        channel: Snowflake,
        messages: Vec<Snowflake>,
    },
    /// Removes a member from `guild`; they may rejoin.
    Kick {
        guild: Snowflake,
        user: Snowflake,
    },
    /// Bans a member, deleting their messages from the last `delete_seconds`.
    Ban {
        guild: Snowflake,
        user: Snowflake,
        delete_seconds: u32,
    },
    /// Times a member out until `until` (an ISO timestamp), or lifts it for `None`.
    Timeout {
        guild: Snowflake,
        user: Snowflake,
        until: Option<String>,
    },
    /// Saves a channel or thread's settings.
    EditChannel {
        channel: Snowflake,
        edit: ChannelEdit,
    },
    /// Opens (or reopens) the DM with `user`. Unselected opens run in the
    /// background, so they never show a captcha and fail silently.
    OpenDm {
        user: Snowflake,
        select: bool,
    },
    /// Joins through an invite; `message` is the one whose invite card was
    /// clicked, if any.
    JoinGuild {
        code: String,
        message: Option<Snowflake>,
    },
    CreateGuild {
        name: String,
    },
    SetGlobalName {
        name: Option<String>,
    },
    /// What the others see: the status and custom status, sent over the
    /// gateway.
    SetPresence(OwnPresence),
    /// Saves one change to the synced settings, rewriting its section of
    /// `current`, the settings as last received.
    EditSettings {
        current: UserSettings,
        edit: SettingsEdit,
    },
    /// The profile, everywhere or in `guild`.
    EditProfile {
        guild: Option<Snowflake>,
        edit: ProfileEdit,
    },
    /// The devices signed in to the account.
    Sessions,
    /// Signs these devices out, by their hashes.
    LogOutSessions(Vec<String>),
    /// The interface language changed; Discord's answers follow it.
    SetLocale(String),
    /// Op 4: joins `channel` (`None` leaves voice) in `guild`, or says we
    /// muted or deafened ourselves there.
    VoiceState {
        guild: Option<Snowflake>,
        channel: Option<Snowflake>,
        self_mute: bool,
        self_deaf: bool,
    },
    /// Starts the call's sound, replacing any running one.
    VoiceConnect(VoiceCall),
    /// Stops the call's sound.
    VoiceDisconnect,
    /// Mute, deafen, devices and volumes, for this call and the next.
    VoiceControls(VoiceControls),
    /// Rings the others in a DM whose call we just started.
    RingCall {
        channel: Snowflake,
    },
    /// Declines a call: our own phone (or these people's) stops ringing.
    StopRinging {
        channel: Snowflake,
        recipients: Vec<Snowflake>,
    },
    /// Gateway op 8: fetch these members' nicknames and roles.
    RequestMembers {
        guild: Snowflake,
        users: Vec<Snowflake>,
    },
    /// Gateway op 8 with a query: members whose name starts with it.
    /// `nonce` comes back on the chunk.
    SearchMembers {
        guild: Snowflake,
        query: String,
        nonce: String,
    },
    /// Gateway op 8 for the message box's `@` suggestions: ten members
    /// whose name starts with `query`.
    QueryMembers {
        guild: Snowflake,
        query: String,
    },
    /// Gateway op 37: what the web client sends on opening a guild, so the
    /// gateway delivers its typing and thread events, and with `members`
    /// the member list rows of a channel in ranges of 100.
    SubscribeGuild {
        guild: Snowflake,
        members: Option<(Snowflake, Vec<[usize; 2]>)>,
    },
    /// Marks `channel` read up to `message`.
    Ack {
        channel: Snowflake,
        message: Snowflake,
    },
    Typing {
        channel: Snowflake,
    },
    /// With a `guild`, the profile as members of that guild see it; `full`
    /// for the full profile, with the servers in common.
    FetchProfile {
        user: Snowflake,
        guild: Option<Snowflake>,
        full: bool,
    },
    /// The profile effect sold as `sku`: its layers and still.
    FetchProfileEffect {
        sku: String,
    },
    /// Downloads an avatar or sticker image.
    FetchAvatar {
        url: String,
    },
    /// Downloads a GIF or WebP and decodes its frames, scaled to fit
    /// `width` by `height` pixels.
    FetchAnimation {
        url: String,
        width: u32,
        height: u32,
    },
    LogOut,
}

/// Which page of a channel's history to load.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    /// The newest messages.
    Latest,
    /// Older than this message.
    Before(Snowflake),
    /// Newer than this message.
    After(Snowflake),
    /// This message with those around it, to jump to it.
    Around(Snowflake),
}

/// A file attached to a message, read into memory when it was picked.
#[derive(Clone)]
pub struct Upload {
    pub name: String,
    pub bytes: Arc<[u8]>,
    /// Hidden behind a click until the reader asks to see it.
    pub spoiler: bool,
    /// Alt text; empty for none.
    pub description: String,
}

/// The message a send answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReplyTo {
    pub message: Snowflake,
    /// Pings the original author.
    pub mention: bool,
}

/// A poll to post, as the creation form fills it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewPoll {
    pub question: String,
    pub answers: Vec<String>,
    pub multiple: bool,
    /// How long it takes votes, in hours.
    pub hours: u32,
}

/// Whose slash commands to list, as the web client asks for them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IndexTarget {
    /// A guild's commands.
    Guild(Snowflake),
    /// A DM's or group DM's commands.
    Channel(Snowflake),
    /// Apps the user installed, usable everywhere.
    User,
}

/// The UI surface a reaction came from, sent as `location` like the web
/// client does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReactionSource {
    /// Clicking a reaction already under the message.
    Message,
    HoverBar,
    Picker,
}

impl ReactionSource {
    pub fn location(self) -> &'static str {
        match self {
            Self::Message => "Message",
            Self::HoverBar => "Message Hover Bar",
            Self::Picker => "Message Reaction Picker",
        }
    }
}
