//! What the UI asked for, as far as reporting the outcome goes.

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
    /// Starting a thread or a forum post.
    CreateThread,
    /// Joining or leaving a thread.
    ThreadMembership,
    /// Running a slash command.
    RunCommand,
    /// Posting a poll.
    CreatePoll,
    /// Changing mutes and notification levels.
    Settings,
    /// Making or changing a group DM, closing a DM, answering a request.
    Channel,
    /// Writing a note on a profile.
    Note,
    /// Leaving a server.
    LeaveGuild,
}
