//! Dispatch events (op 0) into `NetEvent`s. Most map one payload to one
//! event through [`decode`]; READY also starts the session.

use serde::de::DeserializeOwned;
use serde_json::value::RawValue;

use super::{Events, Session, State};
use crate::identity::Identity;
use crate::model::{
    FriendPresence, Guild, NoteUpdate, Presence, Ready, ReadySupplemental, RecipientUpdate, RelationshipRemove, SettingsProtoUpdate,
    UserSettings, guild_folders,
};
use crate::protocol::{Link, NetEvent, ReactionEvent, Snapshot};

/// Sends the event a dispatch of type `kind` stands for, if the UI wants
/// one. Unhandled types are dropped without parsing.
#[hotpath::measure]
pub(super) fn dispatch(
    kind: Option<&str>,
    data: Option<&RawValue>,
    identity: &Identity,
    state: &mut State,
    events: &Events,
) {
    let (Some(kind), Some(data)) = (kind, data) else { return };
    if kind == "USER_SETTINGS_PROTO_UPDATE" {
        for event in settings_update(data.get()) {
            events.try_send(event).ok();
        }
        return;
    }
    if let Some(event) = event(kind, data.get(), identity, state) {
        events.try_send(event).ok();
    }
}

/// One settings update can change both the folders and the settings
/// Emerald reads, so it makes up to two events.
fn settings_update(data: &str) -> Vec<NetEvent> {
    let Some(update) = decode("USER_SETTINGS_PROTO_UPDATE", data, |u: SettingsProtoUpdate| u) else { return vec![] };
    // Only the preloaded settings (type 1) hold them.
    if update.settings.kind != 1 {
        return vec![];
    }
    let proto = &update.settings.proto;
    let mut events = Vec::new();
    // A partial update may leave the folders out.
    if let Some(folders) = guild_folders(proto) {
        events.push(NetEvent::Folders(folders));
    }
    if let Some(settings) = UserSettings::decode(proto) {
        events.push(NetEvent::UserSettings { settings, partial: update.partial });
    }
    events
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
        "USER_NOTE_UPDATE" => decode(kind, data, |n: NoteUpdate| NetEvent::Note { user: n.id, note: n.note }),
        "CHANNEL_RECIPIENT_ADD" | "CHANNEL_RECIPIENT_REMOVE" => decode(kind, data, |r: RecipientUpdate| {
            NetEvent::Recipient {
                channel: r.channel_id,
                user: r.user,
                added: kind == "CHANNEL_RECIPIENT_ADD",
            }
        }),
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
        "MESSAGE_DELETE_BULK" => decode(kind, data, NetEvent::MessageDeleteBulk),
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
        "VOICE_STATE_UPDATE" => decode(kind, data, NetEvent::VoiceState),
        "VOICE_SERVER_UPDATE" => decode(kind, data, NetEvent::VoiceServer),
        "CALL_CREATE" | "CALL_UPDATE" => decode(kind, data, NetEvent::Call),
        "CALL_DELETE" => decode(kind, data, |c: crate::model::CallDelete| NetEvent::CallDelete(c.channel_id)),
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
    let settings = r.user_settings_proto.as_deref().and_then(UserSettings::decode);
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
        settings,
        session_hash: r.auth_session_id_hash,
    }))
}

/// Friend presences only: guild ones come with the member list instead.
fn friend_presences(presences: Vec<Presence>) -> Vec<FriendPresence> {
    presences
        .into_iter()
        .filter_map(|p| {
            Some(FriendPresence {
                user: p.user()?,
                status: p.status,
                activities: p.activities,
            })
        })
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
