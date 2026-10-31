//! The Friends page's "Active Now" column: friends in voice and friends
//! playing or listening, built from presences and voice states as
//! discord.com builds it. Custom statuses do not count.

use emerald_discord::model::{Activity, ActivityTimestamps, Snowflake, activity_kind};

/// One card in the column.
#[derive(Clone, Debug)]
pub enum ActiveCard {
    /// Friends talking in the same voice channel; `guild` is `None` for a call.
    Voice { guild: Option<Snowflake>, channel: Snowflake, users: Vec<Snowflake> },
    /// A friend playing, streaming, listening or watching.
    Activity { user: Snowflake, activity: Activity },
}

/// The cards for `friends`: voice channels first, in the order their first
/// friend appears, then each friend's main activity, newest first.
pub fn active_now(
    friends: &[Snowflake],
    activities: impl Fn(Snowflake) -> Vec<Activity>,
    voice: impl Fn(Snowflake) -> Option<(Option<Snowflake>, Snowflake)>,
) -> Vec<ActiveCard> {
    let mut cards: Vec<ActiveCard> = Vec::new();
    let mut playing: Vec<(Snowflake, Activity)> = Vec::new();
    for &user in friends {
        if let Some((guild, channel)) = voice(user) {
            let existing = cards.iter_mut().find_map(|card| match card {
                ActiveCard::Voice { channel: c, users, .. } if *c == channel => Some(users),
                _ => None,
            });
            match existing {
                Some(users) => users.push(user),
                None => cards.push(ActiveCard::Voice { guild, channel, users: vec![user] }),
            }
            continue;
        }
        if let Some(activity) = activities(user).into_iter().find(|a| a.kind != activity_kind::CUSTOM) {
            playing.push((user, activity));
        }
    }
    let started = |activity: &Activity| activity.timestamps.and_then(|t: ActivityTimestamps| t.start).unwrap_or(0);
    playing.sort_by_key(|(_, activity)| std::cmp::Reverse(started(activity)));
    cards.extend(playing.into_iter().map(|(user, activity)| ActiveCard::Activity { user, activity }));
    cards
}

#[cfg(test)]
mod tests {
    use super::*;

    fn activity(kind: u8, name: &str, start: Option<u64>) -> Activity {
        Activity {
            kind,
            name: name.into(),
            timestamps: start.map(|start| ActivityTimestamps { start: Some(start), end: None }),
            ..Activity::default()
        }
    }

    const A: Snowflake = Snowflake(1);
    const B: Snowflake = Snowflake(2);
    const C: Snowflake = Snowflake(3);
    const D: Snowflake = Snowflake(4);

    #[test]
    fn voice_comes_before_games_and_custom_status_is_ignored() {
        let cards = active_now(
            &[A, B, C],
            |user| match user {
                A => vec![activity(activity_kind::PLAYING, "Overwatch", Some(10))],
                C => vec![activity(activity_kind::CUSTOM, "Custom Status", None)],
                _ => Vec::new(),
            },
            |user| (user == B).then_some((Some(Snowflake(9)), Snowflake(90))),
        );
        assert_eq!(cards.len(), 2);
        assert!(matches!(&cards[0], ActiveCard::Voice { users, .. } if users == &vec![B]));
        assert!(matches!(&cards[1], ActiveCard::Activity { user, .. } if *user == A));
    }

    #[test]
    fn friends_in_the_same_channel_share_a_card() {
        let cards = active_now(&[A, B], |_| Vec::new(), |_| Some((Some(Snowflake(9)), Snowflake(90))));
        assert_eq!(cards.len(), 1);
        assert!(matches!(&cards[0], ActiveCard::Voice { users, .. } if users.len() == 2));
    }

    #[test]
    fn newest_activity_first() {
        let cards = active_now(
            &[A, D],
            |user| vec![activity(activity_kind::PLAYING, "x", Some(if user == A { 10 } else { 20 }))],
            |_| None,
        );
        assert!(matches!(&cards[0], ActiveCard::Activity { user, .. } if *user == D));
    }

    #[test]
    fn nobody_active_means_an_empty_column() {
        assert!(active_now(&[A, B], |_| Vec::new(), |_| None).is_empty());
    }
}
