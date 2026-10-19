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
