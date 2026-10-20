use serde::Deserialize;

use super::Snowflake;

#[derive(Debug, Default, Deserialize)]
pub struct UserProfileDetails {
    #[serde(default)]
    pub bio: Option<String>,
    #[serde(default)]
    pub pronouns: Option<String>,
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
}

/// A user's membership of the guild the profile was requested for.
#[derive(Debug, Default, Deserialize)]
pub struct ProfileMember {
    #[serde(default)]
    pub nick: Option<String>,
    #[serde(default)]
    pub roles: Vec<Snowflake>,
    #[serde(default)]
    pub joined_at: Option<String>,
}

impl ProfileMember {
    /// When they joined the guild, in Unix seconds.
    pub fn joined_secs(&self) -> Option<i64> {
        iso_secs(self.joined_at.as_deref()?)
    }
}

/// Unix seconds of an ISO 8601 timestamp such as `2021-05-01T12:30:00.123000+00:00`.
/// Offsets are ignored: the callers show a calendar day.
fn iso_secs(text: &str) -> Option<i64> {
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
    fn parses_discord_timestamps() {
        assert_eq!(iso_secs("1970-01-01T00:00:00+00:00"), Some(0));
        assert_eq!(iso_secs("2021-05-01T12:30:15.123000+00:00"), Some(1_619_872_215));
        assert_eq!(iso_secs("nonsense"), None);
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
        format!("https://cdn.discordapp.com/badge-icons/{}.png", self.icon)
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
    /// Banner fill when there is no banner image, as `#rrggbb`.
    #[serde(default)]
    pub banner_color: Option<String>,
    #[serde(default)]
    pub accent_color: Option<u32>,
}
