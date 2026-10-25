//! Events from the network thread to the UI.

use super::{Action, IndexTarget, Page, SearchQuery, SearchScope, ThreadQuery, Upload};
use crate::model::{
    Channel, CommandIndex, Emoji, Gif, Guild, GuildFolder, GuildSettings, GuildEmojisUpdate, GuildStickersUpdate, Member, MemberListUpdate,
    MemberUpdate, MembersChunk, Message, MessageAck, MessageDelete, MessageUpdate, PollVote, Profile, ReactionAddMany,
    ReactionClear, ReactionUpdate, Relationship, Invite, RoleDelete, RoleUpdate, Snowflake, StickerPack, ThreadListSync,
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
    /// Someone joined (`added`) or left a group DM.
    Recipient {
        channel: Snowflake,
        user: User,
        added: bool,
    },
    /// The user's note about `user`, read or changed on any device.
    Note {
        user: Snowflake,
        note: String,
    },
    /// Answer to `Command::MutualFriends`; `None` when it failed.
    MutualFriends {
        user: Snowflake,
        friends: Option<Vec<User>>,
    },
    /// Answer to `Command::CreateInvite`.
    InviteCreated {
        channel: Snowflake,
        invite: Result<Invite, String>,
    },
    /// Answer to `Command::ResolveInvite`; `None` for a dead or unknown one.
    Invite {
        code: String,
        invite: Option<Invite>,
    },
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
    /// Answer to `Command::ThreadSearch`.
    ThreadSearch {
        channel: Snowflake,
        query: ThreadQuery,
        offset: usize,
        /// `None` when the request failed.
        page: Option<ThreadPage>,
    },
    /// Answer to `Command::Pins`.
    Pins {
        channel: Snowflake,
        before: Option<String>,
        page: Result<PinPage, String>,
    },
    /// Answer to `Command::Search`.
    SearchResults {
        scope: SearchScope,
        query: SearchQuery,
        offset: usize,
        page: Result<SearchPage, String>,
    },
    /// Answer to `Command::Mentions`.
    Mentions {
        before: Option<Snowflake>,
        messages: Result<Vec<Message>, String>,
    },
    /// A server's notification settings, or the DMs', changed here or on
    /// another device.
    GuildSettings(GuildSettings),
    /// The server list's folders and order changed.
    Folders(Vec<GuildFolder>),
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
    /// Notification settings per server, and the DMs'.
    pub guild_settings: Vec<GuildSettings>,
    /// The server list's folders; `None` when READY had none to give.
    pub folders: Option<Vec<GuildFolder>>,
}

/// Pinned messages, newest pin first.
pub struct PinPage {
    /// Each message with when it was pinned, as Discord writes it.
    pub pins: Vec<(Message, String)>,
    pub has_more: bool,
}

/// A page of search results.
pub struct SearchPage {
    pub messages: Vec<Message>,
    pub total: u32,
    /// Discord is still indexing this server; try again in a moment.
    pub indexing: bool,
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
