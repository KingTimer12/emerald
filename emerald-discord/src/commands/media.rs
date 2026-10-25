//! Pictures and pickers: avatars, animated images, GIFs and stickers.

use super::outcome::{host, or_expired};
use crate::protocol::NetEvent;
use crate::rest::{self, Rest};

pub async fn gifs(rest: &Rest, query: Option<String>) -> NetEvent {
    match rest.gifs(query.as_deref()).await {
        Ok(gifs) => {
            // Counts only: the search is the user's own words.
            tracing::info!(
                "gifs: {} results, {} with an image to show",
                gifs.len(),
                gifs.iter().filter(|gif| gif.image_url().is_some()).count()
            );
            NetEvent::Gifs { query, gifs: Ok(gifs) }
        }
        Err(e) => or_expired(e, |e| {
            let error = rest::describe(&e);
            tracing::warn!("gifs: {error}");
            NetEvent::Gifs { query, gifs: Err(error) }
        }),
    }
}

pub async fn sticker_packs(rest: &Rest) -> NetEvent {
    match rest.sticker_packs().await {
        Ok(packs) => NetEvent::StickerPacks(Some(packs)),
        Err(e) => or_expired(e, |e| {
            tracing::warn!("sticker packs: {}", rest::describe(&e));
            NetEvent::StickerPacks(None)
        }),
    }
}

/// An avatar or sticker image.
pub async fn avatar(rest: &Rest, url: String) -> NetEvent {
    let bytes = rest
        .cdn_image(&url)
        .await
        .map_err(|e| tracing::warn!("avatar {url}: {e:#}"))
        .ok();
    NetEvent::Avatar { url, bytes }
}

/// A GIF or WebP, decoded off the runtime thread into frames that fit
/// `width` by `height`.
pub async fn animation(rest: &Rest, url: String, width: u32, height: u32) -> NetEvent {
    let bytes = rest
        .cdn_animation(&url)
        .await
        .map_err(|e| tracing::warn!("animation from {}: {e:#}", host(&url)))
        .ok();
    let frames = match bytes {
        Some(bytes) => tokio::task::spawn_blocking(move || emerald_core::frames::decode(&bytes, width, height))
            .await
            .ok()
            .flatten(),
        None => None,
    };
    NetEvent::Animation { url, frames }
}
