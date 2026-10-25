//! The web client's `client_build_number`, which changes with every Discord
//! deploy. A stale number stands out and can lose routes gated by build, so
//! startup reads the current one from `window.GLOBAL_ENV` in the
//! `discord.com/app` page, the same document a browser loads before it
//! connects. The last number seen is cached for offline starts.

use std::path::PathBuf;
use std::time::Duration;

use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, HeaderMap, HeaderValue, USER_AGENT};

use super::{identity, urls};

/// Observed on 2026-10-20; used only when the page and the cache both fail.
const FALLBACK: u64 = 643_186;
/// Startup waits for this, so a slow network falls back quickly.
const TIMEOUT: Duration = Duration::from_secs(5);

/// `EMERALD_BUILD_NUMBER`, else the live page, else the cache, else
/// `FALLBACK`. Never older than `FALLBACK`.
pub async fn current(accept_language: &str) -> u64 {
    if let Some(number) = std::env::var("EMERALD_BUILD_NUMBER").ok().and_then(|v| v.parse().ok()) {
        return number;
    }
    match fetch(accept_language).await {
        Ok(number) => {
            if let Some(path) = cache_path()
                && let Err(e) = write(&path, number)
            {
                tracing::warn!("could not cache build number: {e}");
            }
            number.max(FALLBACK)
        }
        Err(e) => {
            tracing::warn!("could not read the current build number: {e}");
            cache_path()
                .and_then(|path| std::fs::read_to_string(path).ok())
                .and_then(|text| text.trim().parse().ok())
                .map_or(FALLBACK, |cached: u64| cached.max(FALLBACK))
        }
    }
}

async fn fetch(accept_language: &str) -> anyhow::Result<u64> {
    // A top-level navigation, as Chrome sends it.
    let mut headers = HeaderMap::new();
    headers.insert(
        ACCEPT,
        HeaderValue::from_static(
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8",
        ),
    );
    headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_str(accept_language)?);
    headers.insert("sec-ch-ua", HeaderValue::from_static(identity::SEC_CH_UA));
    headers.insert("sec-ch-ua-mobile", HeaderValue::from_static("?0"));
    headers.insert("sec-ch-ua-platform", HeaderValue::from_static(identity::SEC_CH_UA_PLATFORM));
    headers.insert("sec-fetch-dest", HeaderValue::from_static("document"));
    headers.insert("sec-fetch-mode", HeaderValue::from_static("navigate"));
    headers.insert("sec-fetch-site", HeaderValue::from_static("none"));
    headers.insert("sec-fetch-user", HeaderValue::from_static("?1"));
    headers.insert("upgrade-insecure-requests", HeaderValue::from_static("1"));
    headers.insert(USER_AGENT, HeaderValue::from_static(identity::USER_AGENT));

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .timeout(TIMEOUT)
        .build()?;
    let url = urls::WEB.join("/app");
    let page = client.get(&url).send().await?.error_for_status()?.text().await?;
    parse(&page).ok_or_else(|| anyhow::anyhow!("no BUILD_NUMBER in {url}"))
}

/// `"BUILD_NUMBER":"643186"` inside `window.GLOBAL_ENV`.
fn parse(page: &str) -> Option<u64> {
    const KEY: &str = "\"BUILD_NUMBER\":\"";
    let start = page.find(KEY)? + KEY.len();
    let digits = &page[start..];
    let end = digits.find('"')?;
    digits[..end].parse().ok()
}

fn cache_path() -> Option<PathBuf> {
    Some(emerald_core::paths::cache_dir()?.join("build_number"))
}

fn write(path: &std::path::Path, number: u64) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, number.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_number_from_global_env() {
        let page = r#"<script>window.GLOBAL_ENV = {"NODE_ENV":"production","BUILT_AT":"1792480825953","HTML_TIMESTAMP":Date.now(),"BUILD_NUMBER":"643186","PROJECT_ENV":"production"}</script>"#;
        assert_eq!(parse(page), Some(643_186));
        assert_eq!(parse("<html></html>"), None);
        assert_eq!(parse(r#""BUILD_NUMBER":"abc""#), None);
    }
}
