//! What people are doing, worded as the web client words it: the custom
//! status, else the game, music or stream.

use emerald_discord::model::{Activity, activity_kind};

/// The line under someone's name: their custom status, else what they are
/// doing.
pub(super) fn activity_text(activities: &[Activity]) -> Option<String> {
    let custom = activities
        .iter()
        .find(|a| a.kind == activity_kind::CUSTOM)
        .and_then(Activity::custom_text);
    custom.or_else(|| activities.iter().find_map(headline))
}

/// "Playing VALORANT", "Listening to Spotify"…; `None` for the custom
/// status and kinds the web client does not show.
pub(super) fn headline(activity: &Activity) -> Option<String> {
    let name = &activity.name;
    Some(match activity.kind {
        activity_kind::PLAYING => t!("activity.playing", name = name),
        activity_kind::STREAMING => t!("activity.streaming", name = name),
        activity_kind::LISTENING => t!("activity.listening", name = name),
        activity_kind::WATCHING => t!("activity.watching", name = name),
        activity_kind::COMPETING => t!("activity.competing", name = name),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use emerald_discord::model::ActivityEmoji;

    fn activity(kind: u8, name: &str, state: Option<&str>) -> Activity {
        Activity {
            kind,
            name: name.into(),
            state: state.map(Into::into),
            ..Default::default()
        }
    }

    #[test]
    fn custom_status_wins_over_games() {
        assert_eq!(
            activity_text(&[
                activity(activity_kind::PLAYING, "VALORANT", None),
                activity(activity_kind::CUSTOM, "Custom Status", Some("ranked")),
            ]),
            Some("ranked".into())
        );
        assert_eq!(
            activity_text(&[activity(activity_kind::LISTENING, "Spotify", None)]),
            Some("Listening to Spotify".into())
        );
        assert_eq!(activity_text(&[]), None);
    }

    #[test]
    fn custom_status_shows_its_emoji() {
        let mut custom = activity(activity_kind::CUSTOM, "Custom Status", Some("oi"));
        custom.emoji = Some(ActivityEmoji {
            name: "👋".into(),
            id: None,
        });
        assert_eq!(activity_text(&[custom.clone()]), Some("👋 oi".into()));
        custom.state = None;
        assert_eq!(activity_text(&[custom]), Some("👋".into()));
    }
}
