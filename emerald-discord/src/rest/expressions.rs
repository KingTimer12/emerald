//! The GIF and sticker pickers.

use anyhow::Result;

use super::Rest;
use super::encoding::encode_component;
use crate::model::{Gif, StickerPack, StickerPacks};
use crate::packets::message;
use emerald_http::{Call, Policy};

impl Rest {
    /// The GIF picker's results, from Discord's Tenor proxy: a search, or
    /// the trending GIFs. Asked for in WebM like the web client on macOS
    /// and Windows; only the still `preview` and `gif_src` are drawn here.
    pub async fn gifs(&self, query: Option<&str>) -> Result<Vec<Gif>> {
        let locale = self.identity.locale();
        let call = match query {
            Some(query) => Call::get(
                "/gifs/search",
                format!("/gifs/search?q={}&media_format=webm&locale={locale}", encode_component(query)),
            ),
            None => Call::get("/gifs/trending-gifs", format!("/gifs/trending-gifs?media_format=webm&locale={locale}")),
        };
        self.api.fetch(call.cached(Policy::MICRO)).await
    }

    /// Discord's standard sticker packs, named in the interface language,
    /// as the sticker picker asks for them.
    pub async fn sticker_packs(&self) -> Result<Vec<StickerPack>> {
        let path = format!("/sticker-packs?locale={}", self.identity.locale());
        let packs: StickerPacks = self.api.fetch(Call::get("/sticker-packs", path).cached(Policy::MICRO)).await?;
        Ok(packs.sticker_packs)
    }

    /// Tells Discord which GIF was picked for `query`, as the web client
    /// does before sending it.
    pub async fn select_gif(&self, id: &str, query: &str) -> Result<()> {
        self.api.run(Call::post("/gifs/select", "/gifs/select").json(message::select_gif(id, query))).await
    }
}
