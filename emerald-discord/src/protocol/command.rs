//! Requests from the UI to the network thread, and the values they carry.

use std::sync::Arc;

use super::{SearchQuery, SearchScope, SettingsChange, ThreadQuery};
use crate::model::{ChannelKind, Emoji, Gif, Snowflake, Status};

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
    /// Opens (or reopens) the DM with `user`. Unselected opens run in the
    /// background, so they never show a captcha and fail silently.
    OpenDm {
        user: Snowflake,
        select: bool,
    },
    JoinGuild {
        code: String,
    },
    CreateGuild {
        name: String,
    },
    SetGlobalName {
        name: Option<String>,
    },
    SetStatus(Status),
    /// The interface language changed; Discord's answers follow it.
    SetLocale(String),
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
    /// With a `guild`, the profile as members of that guild see it.
    FetchProfile {
        user: Snowflake,
        guild: Option<Snowflake>,
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
