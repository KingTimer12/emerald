//! Nitro cosmetics others see on a user: the decoration around the avatar,
//! the nameplate behind the name in lists, and the effect over the profile.
//! Their images come from the collectibles shop by SKU, as the web client
//! builds them (`getCollectiblesItemAssetUrl`).

use serde::Deserialize;

use crate::urls::CDN;

/// `collectibles[].type` in a profile: the effect drawn over it.
pub const PROFILE_EFFECT: u8 = 1;
/// A profile frame; read but not drawn yet.
pub const PROFILE_FRAME: u8 = 3;

/// An item's image in the shop's media; `layer` names one part of a
/// many-layered item. No extension: the CDN answers PNG or WebP.
fn shop_asset(sku: &str, layer: Option<&str>, format: &str) -> String {
    match layer {
        Some(layer) => CDN.join(format_args!("/media/v1/collectibles-shop/{sku}/{layer}/{format}")),
        None => CDN.join(format_args!("/media/v1/collectibles-shop/{sku}/{format}")),
    }
}

/// Whether an item lent until `expires_at` (Unix seconds) is over.
fn expired(expires_at: Option<i64>, now: i64) -> bool {
    expires_at.is_some_and(|at| at < now)
}

/// `avatar_decoration_data`: a frame drawn around the avatar, 1.2 times
/// its size.
#[derive(Clone, Debug, Deserialize)]
pub struct AvatarDecoration {
    #[serde(default)]
    pub sku_id: Option<String>,
    #[serde(default)]
    pub expires_at: Option<i64>,
}

impl AvatarDecoration {
    /// The decoration as a still image, or `animated` (an APNG), unless it
    /// expired by `now`.
    pub fn url(&self, now: i64, animated: bool) -> Option<String> {
        let sku = self.sku_id.as_deref().filter(|sku| !sku.is_empty())?;
        (!expired(self.expires_at, now)).then(|| shop_asset(sku, None, format(animated)))
    }
}

fn format(animated: bool) -> &'static str {
    if animated { "animated" } else { "static" }
}

/// `collectibles` on a user or member.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Collectibles {
    #[serde(default)]
    pub nameplate: Option<Nameplate>,
}

/// An image and a color drawn behind the name in the member list and the
/// DM list.
#[derive(Clone, Debug, Deserialize)]
pub struct Nameplate {
    pub sku_id: String,
    /// One of Discord's named palettes, as `crimson` or `bubble_gum`.
    #[serde(default)]
    pub palette: String,
    #[serde(default)]
    pub expires_at: Option<i64>,
}

impl Nameplate {
    /// Its still image, or `animated` (the web client's choice where it
    /// plays no video), unless it expired by `now`.
    pub fn url(&self, now: i64, animated: bool) -> Option<String> {
        (!self.sku_id.is_empty() && !expired(self.expires_at, now)).then(|| shop_asset(&self.sku_id, None, format(animated)))
    }

    /// The palette's color on a dark or a light theme, from the web
    /// client's table; `None` for a name it does not know.
    pub fn color(&self, dark: bool) -> Option<u32> {
        let (dark_color, light_color) = match self.palette.as_str() {
            "crimson" => (0x900007, 0xE7040F),
            "berry" => (0x893A99, 0xB11FCF),
            "sky" => (0x0080B7, 0x56CCFF),
            "teal" => (0x086460, 0x7DEED7),
            "forest" => (0x2D5401, 0x6AA624),
            "bubble_gum" => (0xDC3E97, 0xF957B3),
            "violet" => (0x730BC8, 0x972FED),
            "cobalt" => (0x0131C2, 0x4278FF),
            "clover" => (0x047B20, 0x63CD5A),
            "lemon" => (0xF6CD12, 0xFED400),
            "white" => (0xFFFFFF, 0xFFFFFF),
            "black" => (0x000000, 0x000000),
            _ => return None,
        };
        Some(if dark { dark_color } else { light_color })
    }
}

/// One of `user_profile.collectibles[]` or `guild_member_profile.collectibles[]`.
#[derive(Clone, Debug, Deserialize)]
pub struct ProfileCollectible {
    pub sku_id: String,
    #[serde(rename = "type")]
    pub kind: u8,
    #[serde(default)]
    pub expires_at: Option<i64>,
}

impl ProfileCollectible {
    pub fn active(&self, now: i64) -> bool {
        !self.sku_id.is_empty() && !expired(self.expires_at, now)
    }
}

/// `GET /collectibles-products/{sku}`: what a profile effect draws.
#[derive(Debug, Deserialize)]
pub struct CollectiblesProduct {
    #[serde(default)]
    pub items: Vec<ProductItem>,
}

/// An item of a product; for an effect, its layers and stills. Keys are
/// camelCase here, as the shop sends them.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductItem {
    #[serde(rename = "type", default)]
    pub kind: u8,
    #[serde(default)]
    pub reduced_motion_src: Option<String>,
    #[serde(default)]
    pub static_frame_src: Option<String>,
    #[serde(default)]
    pub effects: Vec<EffectLayer>,
}

/// One animated image of a profile effect, placed over the profile and
/// played on a clock in milliseconds from when the profile opens.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectLayer {
    pub src: String,
    #[serde(default)]
    pub r#loop: bool,
    /// The pause between loops.
    #[serde(default)]
    pub loop_delay: u64,
    #[serde(default)]
    pub start: u64,
    #[serde(default)]
    pub duration: u64,
    #[serde(default)]
    pub position: Position,
    #[serde(default)]
    pub z_index: i32,
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
pub struct Position {
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
}

impl EffectLayer {
    /// Whether the layer shows `elapsed` milliseconds after the profile
    /// opened: from its start for its duration, then again after each
    /// pause when it loops.
    pub fn shown(&self, elapsed: u64) -> bool {
        let Some(local) = elapsed.checked_sub(self.start) else { return false };
        if self.duration == 0 {
            return true;
        }
        if self.r#loop {
            local % (self.duration + self.loop_delay) < self.duration
        } else {
            local < self.duration
        }
    }

    /// Whether the layer will still appear or vanish after `elapsed`, so
    /// the profile must keep redrawing on its own.
    pub fn changes_after(&self, elapsed: u64) -> bool {
        if self.r#loop {
            self.loop_delay > 0 || elapsed < self.start
        } else {
            self.duration > 0 && elapsed < self.start + self.duration
        }
    }
}

/// A profile effect: its layers, lowest first, and the still the web
/// client shows with reduced motion.
#[derive(Clone, Debug)]
pub struct ProfileEffect {
    pub still: Option<String>,
    pub layers: Vec<EffectLayer>,
}

impl CollectiblesProduct {
    /// The profile effect this product is, if it is one.
    pub fn effect(self) -> Option<ProfileEffect> {
        let item = self.items.into_iter().find(|item| item.kind == PROFILE_EFFECT)?;
        let web = |src: &String| src.starts_with("https://");
        let still = item.reduced_motion_src.or(item.static_frame_src).filter(web);
        let mut layers: Vec<EffectLayer> = item.effects.into_iter().filter(|layer| web(&layer.src)).collect();
        layers.sort_by_key(|layer| layer.z_index);
        (still.is_some() || !layers.is_empty()).then_some(ProfileEffect { still, layers })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shop_images_come_by_sku_until_they_expire() {
        let decoration: AvatarDecoration = serde_json::from_str(r#"{"asset":"a_1","sku_id":"42","expires_at":null}"#).unwrap();
        assert_eq!(
            decoration.url(0, false).as_deref(),
            Some("https://cdn.discordapp.com/media/v1/collectibles-shop/42/static")
        );
        assert_eq!(
            decoration.url(0, true).as_deref(),
            Some("https://cdn.discordapp.com/media/v1/collectibles-shop/42/animated")
        );
        let lent: AvatarDecoration = serde_json::from_str(r#"{"sku_id":"42","expires_at":100}"#).unwrap();
        assert!(lent.url(99, false).is_some());
        assert!(lent.url(101, false).is_none());
        let plate: Nameplate = serde_json::from_str(r#"{"sku_id":"7","palette":"berry","label":"x","asset":"y"}"#).unwrap();
        assert_eq!(plate.color(true), Some(0x893A99));
        assert_eq!(plate.url(0, false).as_deref(), Some("https://cdn.discordapp.com/media/v1/collectibles-shop/7/static"));
    }

    #[test]
    fn products_name_their_effect() {
        let product: CollectiblesProduct = serde_json::from_str(
            r#"{"items":[{"sku_id":"1","type":1,"reducedMotionSrc":"https://cdn/e.png","effects":[
                {"src":"https://cdn/loop.png","loop":true,"loopDelay":1000,"start":500,"duration":2000,"position":{"x":0,"y":10},"zIndex":2},
                {"src":"https://cdn/intro.png","loop":false,"start":0,"duration":1500,"position":{"x":0,"y":0},"zIndex":1}
            ]}]}"#,
        )
        .unwrap();
        let effect = product.effect().unwrap();
        assert_eq!(effect.still.as_deref(), Some("https://cdn/e.png"));
        let [intro, looping] = effect.layers.as_slice() else { panic!("two layers") };
        assert_eq!(intro.src, "https://cdn/intro.png");
        // The intro plays once.
        assert!(intro.shown(0) && intro.shown(1499) && !intro.shown(1500));
        assert!(intro.changes_after(1000) && !intro.changes_after(1500));
        // The loop starts late, shows for two seconds, pauses for one.
        assert!(!looping.shown(499) && looping.shown(500) && looping.shown(2499));
        assert!(!looping.shown(2500) && looping.shown(3500));
        let frame: CollectiblesProduct = serde_json::from_str(r#"{"items":[{"type":3}]}"#).unwrap();
        assert!(frame.effect().is_none());
    }
}
