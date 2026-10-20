//! The messages between the UI and the network thread.

use super::model::{
    Channel, Emoji, Guild, GuildEmojisUpdate, Member, MemberListUpdate, MemberUpdate, MembersChunk, Message,
    MessageAck, MessageDelete, MessageUpdate, Profile, ReactionAddMany, ReactionClear, ReactionUpdate, Relationship,
    RoleDelete, RoleUpdate, Snowflake, Status, TypingStart, User,
};

/// A user-initiated REST action whose outcome the UI reports back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    FriendRequest,
    Relationship,
    JoinGuild,
    CreateGuild,
    UpdateProfile,
    /// Editing, deleting or pinning a message.
    Message,
}

/// The gateway's reaction events.
pub enum ReactionEvent {
    Add(ReactionUpdate),
    Remove(ReactionUpdate),
    AddMany(ReactionAddMany),
    /// Every reaction, or every reaction with one emoji, was removed.
    Clear(ReactionClear),
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

/// The gateway connection's state, worded by the UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Link {
    Connecting,
    Resuming,
    Connected,
    /// Dropped; the next attempt is this many seconds away.
    Reconnecting(u64),
}

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
    ActionDone(Action),
    ActionFailed {
        action: Action,
        error: String,
    },
    GuildCreate(Guild),
    GuildDelete(Snowflake),
    ChannelCreate(Channel),
    ChannelDelete(Channel),
    MessageCreate(Message),
    MessageUpdate(MessageUpdate),
    MessageDelete(MessageDelete),
    Reaction(ReactionEvent),
    /// Our reaction did not go through; the UI undoes it.
    ReactionFailed {
        channel: Snowflake,
        message: Snowflake,
        emoji: Emoji,
        add: bool,
        error: String,
    },
    GuildEmojis(GuildEmojisUpdate),
    TypingStart(TypingStart),
    Profile {
        /// The guild whose member profile was asked for, if any.
        guild: Option<Snowflake>,
        profile: Profile,
    },
    History {
        channel: Snowflake,
        before: Option<Snowflake>,
        messages: Vec<Message>,
    },
    HistoryFailed {
        channel: Snowflake,
        forbidden: bool,
        error: String,
    },
    /// The message with `nonce` was not sent; `content` lets the UI give
    /// the text back to the user.
    SendFailed {
        channel: Snowflake,
        nonce: String,
        content: String,
        error: String,
    },
}

/// The message a send answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReplyTo {
    pub message: Snowflake,
    /// Pings the original author.
    pub mention: bool,
}

/// Requests flowing from the UI to the network thread.
pub enum Command {
    History {
        channel: Snowflake,
        before: Option<Snowflake>,
    },
    Send {
        channel: Snowflake,
        content: String,
        nonce: String,
        reply: Option<ReplyTo>,
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
    LogOut,
}
