//! Dispatch events (op 0) into `NetEvent`s. Most map one payload to one
//! event through [`decode`]; READY also starts the session.

use serde::de::DeserializeOwned;
use serde_json::value::RawValue;

use super::{Events, Session, State};
use crate::identity::Identity;
use crate::model::{
    Guild, Presence, Ready, ReadySupplemental, RelationshipRemove, SettingsProtoUpdate, Snowflake, guild_folders,
};
use crate::protocol::{Link, NetEvent, ReactionEvent, Snapshot};

/// Sends the event a dispatch of type `kind` stands for, if the UI wants
/// one. Unhandled types are dropped without parsing.
pub(super) fn dispatch(
    kind: Option<&str>,
    data: Option<&RawValue>,
    identity: &Identity,
    state: &mut State,
    events: &Events,
) {
    let (Some(kind), Some(data)) = (kind, data) else { return };
    if let Some(event) = event(kind, data.get(), identity, state) {
        events.try_send(event).ok();
    }
}

fn event(kind: &str, data: &str, identity: &Identity, state: &mut State) -> Option<NetEvent> {
    match kind {
        "READY" => Some(ready(decode(kind, data, std::convert::identity)?, identity, state)),
        "RESUMED" => {
            tracing::info!("gateway: resumed");
            state.ready = true;
            Some(NetEvent::Status(Link::Connected))
        }
        "READY_SUPPLEMENTAL" => decode(kind, data, |r: ReadySupplemental| {
            NetEvent::Presences(friend_presences(r.merged_presences.friends))
        }),
        "PRESENCE_UPDATE" => {
            let presence: Presence = decode(kind, data, std::convert::identity)?;
            presence.guild_id.is_none().then(|| NetEvent::Presences(friend_presences(vec![presence])))
        }
        "RELATIONSHIP_ADD" | "RELATIONSHIP_UPDATE" => decode(kind, data, NetEvent::RelationshipAdd),
        "RELATIONSHIP_REMOVE" => decode(kind, data, |r: RelationshipRemove| NetEvent::RelationshipRemove(r.id)),
        "USER_UPDATE" => decode(kind, data, NetEvent::UserUpdate),
        "GUILD_CREATE" => decode(kind, data, NetEvent::GuildCreate),
        "GUILD_DELETE" => {
            let guild: Guild = decode(kind, data, std::convert::identity)?;
            (!guild.unavailable).then_some(NetEvent::GuildDelete(guild.id))
        }
        // An update replaces the channel, which a create also does.
        "CHANNEL_CREATE" | "CHANNEL_UPDATE" | "THREAD_CREATE" | "THREAD_UPDATE" => {
            decode(kind, data, NetEvent::ChannelCreate)
        }
        "CHANNEL_DELETE" | "THREAD_DELETE" => decode(kind, data, NetEvent::ChannelDelete),
        "THREAD_LIST_SYNC" => decode(kind, data, NetEvent::ThreadListSync),
        // Only ever about the current user.
        "THREAD_MEMBER_UPDATE" => decode(kind, data, NetEvent::ThreadJoined),
        "THREAD_MEMBERS_UPDATE" => decode(kind, data, NetEvent::ThreadMembers),
        "MESSAGE_CREATE" => decode(kind, data, |m| NetEvent::MessageCreate(Box::new(m))),
        "MESSAGE_UPDATE" => decode(kind, data, NetEvent::MessageUpdate),
        "MESSAGE_DELETE" => decode(kind, data, NetEvent::MessageDelete),
        "MESSAGE_ACK" => decode(kind, data, NetEvent::MessageAck),
        "GUILD_MEMBERS_CHUNK" => decode(kind, data, NetEvent::MembersChunk),
        "GUILD_MEMBER_LIST_UPDATE" => decode(kind, data, NetEvent::MemberList),
        "GUILD_MEMBER_UPDATE" => decode(kind, data, NetEvent::MemberUpdate),
        "GUILD_ROLE_CREATE" | "GUILD_ROLE_UPDATE" => decode(kind, data, NetEvent::RoleUpdate),
        "GUILD_ROLE_DELETE" => decode(kind, data, NetEvent::RoleDelete),
        "GUILD_EMOJIS_UPDATE" => decode(kind, data, NetEvent::GuildEmojis),
        "GUILD_STICKERS_UPDATE" => decode(kind, data, NetEvent::GuildStickers),
        "MESSAGE_REACTION_ADD" => decode(kind, data, |r| NetEvent::Reaction(ReactionEvent::Add(r))),
        "MESSAGE_REACTION_REMOVE" => decode(kind, data, |r| NetEvent::Reaction(ReactionEvent::Remove(r))),
        "MESSAGE_REACTION_ADD_MANY" => decode(kind, data, |r| NetEvent::Reaction(ReactionEvent::AddMany(r))),
        "MESSAGE_REACTION_REMOVE_ALL" | "MESSAGE_REACTION_REMOVE_EMOJI" => {
            decode(kind, data, |r| NetEvent::Reaction(ReactionEvent::Clear(r)))
        }
        "MESSAGE_POLL_VOTE_ADD" => decode(kind, data, |vote| NetEvent::PollVote { vote, add: true }),
        "MESSAGE_POLL_VOTE_REMOVE" => decode(kind, data, |vote| NetEvent::PollVote { vote, add: false }),
        "TYPING_START" => decode(kind, data, NetEvent::TypingStart),
        "USER_GUILD_SETTINGS_UPDATE" => decode(kind, data, NetEvent::GuildSettings),
        "USER_SETTINGS_PROTO_UPDATE" => {
            let update: SettingsProtoUpdate = decode(kind, data, std::convert::identity)?;
            // Only the preloaded settings hold folders, and a partial update
            // may leave them out.
            (update.settings.kind == 1)
                .then(|| guild_folders(&update.settings.proto))
                .flatten()
                .map(NetEvent::Folders)
        }
        _ => None,
    }
}

/// `data` as a `T`, made into an event by `wrap`. Malformed payloads are
/// logged and skipped.
fn decode<T: DeserializeOwned, E>(kind: &str, data: &str, wrap: impl FnOnce(T) -> E) -> Option<E> {
    serde_json::from_str(data)
        .map_err(|e| tracing::warn!("skipping malformed {kind}: {e}"))
        .ok()
        .map(wrap)
}

/// READY starts the session: a new token if Discord rotated it, the id to
/// resume with, and a snapshot of the account for the UI.
fn ready(r: Ready, identity: &Identity, state: &mut State) -> NetEvent {
    if let Some(token) = r.auth_token {
        identity.replace_token(token);
    }
    tracing::info!(
        "READY: {} guilds, {} private channels, {} users, {} relationships",
        r.guilds.len(),
        r.private_channels.len(),
        r.users.len(),
        r.relationships.len(),
    );
    identity.set_session_id(r.session_id.clone());
    state.session = Some(Session {
        id: r.session_id,
        resume_url: r.resume_gateway_url,
    });
    state.ready = true;
    let members = r
        .guilds
        .iter()
        .zip(r.merged_members)
        .filter_map(|(guild, mut members)| Some((guild.id, members.pop()?)))
        .collect();
    let read_states = r
        .read_state
        .entries
        .into_iter()
        .filter(|state| state.read_state_type == 0)
        .filter_map(|state| Some((state.id, state.last_message_id?, state.mention_count)))
        .collect();
    let folders = r.user_settings_proto.as_deref().and_then(guild_folders);
    NetEvent::Ready(Box::new(Snapshot {
        me: r.user,
        guilds: r.guilds,
        dms: r.private_channels,
        users: r.users,
        relationships: r.relationships,
        members,
        read_states,
        guild_settings: r.user_guild_settings.entries,
        folders,
    }))
}

/// Friends' statuses as `(user, status)`.
fn friend_presences(presences: Vec<Presence>) -> Vec<(Snowflake, String)> {
    presences
        .into_iter()
        .filter_map(|p| Some((p.user()?, p.status)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event_of(kind: &str, data: &str) -> Option<NetEvent> {
        let identity = Identity::new(crate::token::Token::new("t".into()).unwrap(), 1);
        event(kind, data, &identity, &mut State::default())
    }

    #[test]
    fn unknown_and_malformed_dispatches_are_dropped() {
        assert!(event_of("SOMETHING_NEW", "{}").is_none());
        assert!(event_of("MESSAGE_DELETE", "not json").is_none());
    }

    #[test]
    fn unavailable_guilds_are_not_deleted() {
        assert!(event_of("GUILD_DELETE", r#"{"id":"1","unavailable":true}"#).is_none());
        assert!(matches!(event_of("GUILD_DELETE", r#"{"id":"1"}"#), Some(NetEvent::GuildDelete(_))));
    }

    #[test]
    fn poll_votes_keep_their_direction() {
        let vote = r#"{"user_id":"1","channel_id":"2","message_id":"3","answer_id":1}"#;
        assert!(matches!(event_of("MESSAGE_POLL_VOTE_REMOVE", vote), Some(NetEvent::PollVote { add: false, .. })));
    }
}
