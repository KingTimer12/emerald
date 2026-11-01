use serde::Deserialize;

use super::{AvatarDecoration, Collectibles, ProfileCollectible, Snowflake, User, Widget};
use crate::urls::CDN;

/// The profile's own fields, global (`user_profile`) or for one server
/// (`guild_member_profile`); also what a profile PATCH answers.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct UserProfileDetails {
    #[serde(default)]
    pub bio: Option<String>,
    #[serde(default)]
    pub pronouns: Option<String>,
    /// Banner image hash.
    #[serde(default)]
    pub banner: Option<String>,
    #[serde(default)]
    pub accent_color: Option<u32>,
    /// The profile effect and frame shown, by SKU.
    #[serde(default)]
    pub collectibles: Vec<ProfileCollectible>,
    /// The card's two colors, primary then accent, as RGB.
    #[serde(default, deserialize_with = "super::lenient")]
    pub theme_colors: Option<[u32; 2]>,
}

/// `GET /users/{id}/profile`, as the DM profile panel uses it.
#[derive(Debug, Deserialize)]
pub struct Profile {
    pub user: ProfileUser,
    #[serde(default)]
    pub user_profile: UserProfileDetails,
    #[serde(default)]
    pub badges: Vec<Badge>,
    #[serde(default)]
    pub connected_accounts: Vec<ConnectedAccount>,
    /// Present when asked for with a `guild_id`: the user's membership there.
    #[serde(default)]
    pub guild_member: Option<ProfileMember>,
    /// Bio and pronouns the user set for that guild only.
    #[serde(default)]
    pub guild_member_profile: Option<UserProfileDetails>,
    /// Servers both share; only when asked for.
    #[serde(default)]
    pub mutual_guilds: Option<Vec<MutualGuild>>,
    #[serde(default)]
    pub mutual_friends_count: Option<u32>,
    /// Friends both share, sorted by username; only when asked for.
    #[serde(default, deserialize_with = "super::lenient")]
    pub mutual_friends: Option<Vec<User>>,
    /// The Board; kinds Emerald does not draw are skipped there.
    #[serde(default, deserialize_with = "super::lenient_list")]
    pub widgets: Vec<Widget>,
}

/// A server the user shares with someone, with their nickname there.
#[derive(Clone, Debug, Deserialize)]
pub struct MutualGuild {
    pub id: Snowflake,
    #[serde(default)]
    pub nick: Option<String>,
}

/// A user's membership of the guild the profile was requested for.
#[derive(Debug, Default, Deserialize)]
pub struct ProfileMember {
    #[serde(default)]
    pub nick: Option<String>,
    /// Their avatar in that guild only.
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub roles: Vec<Snowflake>,
    #[serde(default)]
    pub joined_at: Option<String>,
}

/// A user's banner image. Animated ones (`a_…`) show their first frame.
pub fn banner_url(user: Snowflake, hash: &str) -> String {
    CDN.join(format_args!("/banners/{user}/{hash}.png?size=600"))
}

/// A user's banner in one guild.
pub fn guild_banner_url(guild: Snowflake, user: Snowflake, hash: &str) -> String {
    CDN.join(format_args!("/guilds/{guild}/users/{user}/banners/{hash}.png?size=600"))
}

/// A user's avatar in one guild.
pub fn guild_avatar_url(guild: Snowflake, user: Snowflake, hash: &str) -> String {
    CDN.join(format_args!("/guilds/{guild}/users/{user}/avatars/{hash}.png?size=128"))
}

impl ProfileMember {
    /// When they joined the guild, in Unix seconds.
    pub fn joined_secs(&self) -> Option<i64> {
        iso_secs(self.joined_at.as_deref()?)
    }
}

/// Unix seconds of an ISO 8601 timestamp such as `2021-05-01T12:30:00.123000+00:00`.
/// Offsets are ignored: the callers show a calendar day.
pub fn iso_secs(text: &str) -> Option<i64> {
    let (date, time) = text.split_once('T')?;
    let mut date = date.split('-').map(|n| n.parse::<i64>().ok());
    let (year, month, day) = (date.next()??, date.next()??, date.next()??);
    let mut time = time.split(|c: char| !c.is_ascii_digit());
    let mut part = || time.next().and_then(|n| n.parse::<i64>().ok()).unwrap_or(0);
    let (hour, minute, second) = (part(), part(), part());
    // Days since 1970-01-01 for a proleptic Gregorian date.
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let doy = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + hour * 3600 + minute * 60 + second)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_profile_fields_parse_and_bad_ones_are_dropped() {
        let profile: Profile = serde_json::from_str(
            r#"{"user":{"id":"1","username":"a","primary_guild":{"identity_guild_id":"9","identity_enabled":true,"tag":"T","badge":"b"},
            "display_name_styles":{"font_id":4,"effect_id":1,"colors":[255]}},
            "user_profile":{"theme_colors":[2105376,15790320]},
            "mutual_friends":[{"id":"2","username":"b"}],
            "widgets":[{"id":"w","data":{"type":"some_future_widget","x":1}},{"data":{"type":"played_games","games":[]}},7]}"#,
        )
        .unwrap();
        assert_eq!(profile.user_profile.theme_colors, Some([0x202020, 0xf0f0f0]));
        assert!(profile.user.user().unwrap().primary_guild.unwrap().shown());
        assert_eq!(profile.mutual_friends.unwrap().len(), 1);
        assert_eq!(profile.widgets.len(), 2);
        assert_eq!(profile.widgets[0].kind(), "some_future_widget");
        // A malformed field leaves only itself empty.
        let profile: Profile =
            serde_json::from_str(r#"{"user":{"id":"1","display_name_styles":"oops"},"user_profile":{"theme_colors":[1]}}"#).unwrap();
        assert!(profile.user.display_name_styles.is_none());
        assert!(profile.user_profile.theme_colors.is_none());
    }

    #[test]
    fn roles_and_relationships_carry_icons_and_since() {
        let role: super::super::Role =
            serde_json::from_str(r#"{"id":"3","name":"Mod","icon":"abc","unicode_emoji":null}"#).unwrap();
        assert_eq!(role.icon.as_deref(), Some("abc"));
        assert_eq!(
            super::super::role_icon_url(role.id, "abc", 20),
            "https://cdn.discordapp.com/role-icons/3/abc.webp?size=20&quality=lossless"
        );
        let friend: super::super::Relationship =
            serde_json::from_str(r#"{"id":"4","type":1,"since":"2020-05-01T10:00:00.000000+00:00"}"#).unwrap();
        assert_eq!(friend.since.as_deref(), Some("2020-05-01T10:00:00.000000+00:00"));
    }

    #[test]
    fn parses_discord_timestamps() {
        assert_eq!(iso_secs("1970-01-01T00:00:00+00:00"), Some(0));
        assert_eq!(iso_secs("2021-05-01T12:30:15.123000+00:00"), Some(1_619_872_215));
        assert_eq!(iso_secs("nonsense"), None);
    }

    #[test]
    fn profile_user_names_who_it_is() {
        let full: ProfileUser =
            serde_json::from_str(r#"{"id":"7","username":"ana","global_name":"Ana","avatar":"abc"}"#).unwrap();
        let user = full.user().unwrap();
        assert_eq!((user.display_name(), user.avatar.as_deref()), ("Ana", Some("abc")));
        let bare: ProfileUser = serde_json::from_str(r#"{"id":"7"}"#).unwrap();
        assert!(bare.user().is_none());
    }
}

#[derive(Debug, Deserialize)]
pub struct Badge {
    /// Hover text, already localized.
    pub description: String,
    /// Icon hash under `badge-icons/`.
    pub icon: String,
}

impl Badge {
    pub fn icon_url(&self) -> String {
        CDN.join(format_args!("/badge-icons/{}.png", self.icon))
    }
}

#[derive(Debug, Deserialize)]
pub struct ConnectedAccount {
    /// Service, e.g. `github` or `steam`.
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct ProfileUser {
    pub id: Snowflake,
    /// Who the profile is of; the people store may not know them yet, as
    /// when it was opened from the member list.
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub global_name: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub bot: bool,
    #[serde(default)]
    pub avatar_decoration_data: Option<AvatarDecoration>,
    #[serde(default)]
    pub collectibles: Option<Collectibles>,
    /// Banner image hash; Nitro only.
    #[serde(default)]
    pub banner: Option<String>,
    /// Banner fill when there is no banner image, as `#rrggbb`.
    #[serde(default)]
    pub banner_color: Option<String>,
    #[serde(default)]
    pub accent_color: Option<u32>,
    #[serde(default, deserialize_with = "super::lenient")]
    pub primary_guild: Option<super::PrimaryGuild>,
    #[serde(default, deserialize_with = "super::lenient")]
    pub display_name_styles: Option<super::NameStyle>,
}

impl ProfileUser {
    /// The user as the rest of the app knows people; `None` when the
    /// answer left out their name.
    pub fn user(&self) -> Option<User> {
        Some(User {
            id: self.id,
            username: self.username.clone()?,
            global_name: self.global_name.clone(),
            avatar: self.avatar.clone(),
            email: None,
            bot: self.bot,
            premium_type: None,
            avatar_decoration_data: self.avatar_decoration_data.clone(),
            collectibles: self.collectibles.clone(),
            primary_guild: self.primary_guild.clone(),
            display_name_styles: self.display_name_styles.clone(),
        })
    }
}
