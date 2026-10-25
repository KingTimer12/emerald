//! Events from the network thread to the UI.

use super::{Action, IndexTarget, Page, Upload};
use crate::model::{
    Channel, CommandIndex, Emoji, Gif, Guild, GuildEmojisUpdate, GuildStickersUpdate, Member, MemberListUpdate,
    MemberUpdate, MembersChunk, Message, MessageAck, MessageDelete, MessageUpdate, PollVote, Profile, ReactionAddMany,
    ReactionClear, ReactionUpdate, Relationship, RoleDelete, RoleUpdate, Snowflake, StickerPack, ThreadListSync,
    ThreadMemberEntry, ThreadMembersUpdate, TypingStart, User,
};

/// Events flowing from the network thread to the UI.
pub enum NetEvent {
    Status(Link),
    Fatal(String),
    /// The token was rejected (gateway 4004 / REST 401); sign in again.
    AuthExpired,
    /// Answer to `Command::LogOut`; `error` is set if the server-side
    /// revocation failed (the local sign-out still happens).
    LoggedOut {
        error: Option<String>,
    },
    Ready(Box<Snapshot>),
    MembersChunk(MembersChunk),
    /// Rows of a lazily loaded member list.
    MemberList(MemberListUpdate),
    /// A member's nickname or roles changed.
    MemberUpdate(MemberUpdate),
    /// A role was created or changed.
    RoleUpdate(RoleUpdate),
    RoleDelete(RoleDelete),
    /// A channel was read up to `message_id`, here or on another device.
    MessageAck(MessageAck),
    /// Friend presences as `(user, status)`.
    Presences(Vec<(Snowflake, String)>),
    RelationshipAdd(Relationship),
    RelationshipRemove(Snowflake),
    UserUpdate(User),
    /// A DM opened with `Command::OpenDm`; selected when `select` is set.
    DmOpened {
        channel: Channel,
        select: bool,
    },
    /// Image bytes (avatar or sticker) for `url`; `None` if the download failed.
    Avatar {
        url: String,
        bytes: Option<Vec<u8>>,
    },
    /// The frames of an animated image asked for with `FetchAnimation`;
    /// `None` if it could not be downloaded or decoded.
    Animation {
        url: String,
        frames: Option<Vec<image::Frame>>,
    },
    ActionDone(Action),
    ActionFailed {
        action: Action,
        error: String,
    },
    GuildCreate(Guild),
    GuildDelete(Snowflake),
    /// Also threads, created or updated.
    ChannelCreate(Channel),
    ChannelDelete(Channel),
    ThreadListSync(ThreadListSync),
    /// We joined a thread, or our membership changed.
    ThreadJoined(ThreadMemberEntry),
    ThreadMembers(ThreadMembersUpdate),
    /// Answer to `Command::CreateThread` or `Command::CreatePost`: the new
    /// thread, to open.
    ThreadCreated(Channel),
    /// Answer to `Command::ThreadMembership`, before the gateway says so.
    ThreadMembership {
        thread: Snowflake,
        joined: bool,
    },
    /// Boxed: a message is far larger than any other event.
    MessageCreate(Box<Message>),
    MessageUpdate(MessageUpdate),
    MessageDelete(MessageDelete),
    Reaction(ReactionEvent),
    /// Someone voted on a poll (`add`) or took a vote back.
    PollVote {
        vote: PollVote,
        add: bool,
    },
    /// Our reaction did not go through; the UI undoes it.
    ReactionFailed {
        channel: Snowflake,
        message: Snowflake,
        emoji: Emoji,
        add: bool,
        error: String,
    },
    GuildEmojis(GuildEmojisUpdate),
    GuildStickers(GuildStickersUpdate),
    /// Answer to `Command::CommandIndex`; `None` when it could not be had.
    CommandIndex {
        target: IndexTarget,
        index: Option<CommandIndex>,
    },
    /// Answer to `Command::StickerPacks`; `None` when it failed.
    StickerPacks(Option<Vec<StickerPack>>),
    /// Answer to `Command::Gifs`: trending ones when `query` is `None`.
    Gifs {
        query: Option<String>,
        gifs: Result<Vec<Gif>, String>,
    },
    TypingStart(TypingStart),
    Profile {
        /// The guild whose member profile was asked for, if any.
        guild: Option<Snowflake>,
        profile: Profile,
    },
    History {
        channel: Snowflake,
        page: Page,
        messages: Vec<Message>,
    },
    HistoryFailed {
        channel: Snowflake,
        forbidden: bool,
        error: String,
    },
    /// First messages of forum posts; those left out have none.
    ForumPosts {
        forum: Snowflake,
        first: Vec<(Snowflake, Message)>,
    },
    ArchivedPosts {
        forum: Snowflake,
        offset: usize,
        /// `None` when the request failed.
        page: Option<ThreadPage>,
    },
    /// The message with `nonce` was not sent; `content` lets the UI give
    /// the text back to the user.
    SendFailed {
        channel: Snowflake,
        nonce: String,
        content: String,
        /// Handed back with the text, to try again.
        files: Vec<Upload>,
        error: String,
    },
}

/// The gateway connection's state, worded by the UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Link {
    Connecting,
    Resuming,
    Connected,
    /// Dropped; the next attempt is this many seconds away.
    Reconnecting(u64),
}

/// The account as READY describes it, when a session starts.
pub struct Snapshot {
    pub me: User,
    pub guilds: Vec<Guild>,
    pub dms: Vec<Channel>,
    pub users: Vec<User>,
    pub relationships: Vec<Relationship>,
    /// The current user's membership per guild.
    pub members: Vec<(Snowflake, Member)>,
    /// Per channel: the last read message and the unread mentions.
    pub read_states: Vec<(Snowflake, Snowflake, u32)>,
}

/// The gateway's reaction events.
pub enum ReactionEvent {
    Add(ReactionUpdate),
    Remove(ReactionUpdate),
    AddMany(ReactionAddMany),
    /// Every reaction, or every reaction with one emoji, was removed.
    Clear(ReactionClear),
}

/// Archived forum posts, with our memberships and their first messages.
pub struct ThreadPage {
    pub threads: Vec<Channel>,
    pub first: Vec<Message>,
    pub has_more: bool,
}
