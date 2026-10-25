//! Discord's hosts, each written once. The REST client joins request paths
//! to [`API`]; models build CDN links from [`CDN`] and [`MEDIA`].

use emerald_http::BaseUrl;

/// API version in REST paths and the gateway query, as the web client.
pub const API_VERSION: u8 = 9;

/// The web client's origin: `Origin`, `Referer` and its pages.
pub const WEB: BaseUrl = BaseUrl::new("https://discord.com");
/// REST API; its version matches [`API_VERSION`] (checked by a test).
pub const API: BaseUrl = BaseUrl::new("https://discord.com/api/v9");
/// Avatars, icons, emoji, badges and Lottie stickers.
pub const CDN: BaseUrl = BaseUrl::new("https://cdn.discordapp.com");
/// The media proxy: resized stickers and attachments.
pub const MEDIA: BaseUrl = BaseUrl::new("https://media.discordapp.net");
/// Where a fresh gateway session connects.
pub const GATEWAY: BaseUrl = BaseUrl::new("wss://gateway.discord.gg");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_names_its_version() {
        assert_eq!(API.as_str(), WEB.join(format_args!("/api/v{API_VERSION}")));
    }
}
