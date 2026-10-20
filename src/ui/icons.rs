//! Monochrome SVG icons bundled into the binary. gpui paints them with the
//! svg element's own text color (black when unset); it is not inherited.

use std::borrow::Cow;

use gpui::{AssetSource, SharedString, Svg, prelude::*, px, rgb, svg};

pub const FRIENDS: &str = "icons/friends.svg";
pub const PROFILE: &str = "icons/profile.svg";
pub const TEXT: &str = "icons/text.svg";
pub const VOICE: &str = "icons/voice.svg";
pub const ANNOUNCEMENT: &str = "icons/announcement.svg";
pub const FORUM: &str = "icons/forum.svg";
pub const STAGE: &str = "icons/stage.svg";
pub const CHEVRON: &str = "icons/chevron.svg";
/// App mark, an emerald cut seen from above; stands in for the DM space.
pub const EMERALD: &str = "icons/emerald.svg";

const ALL: [(&str, &[u8]); 9] = [
    (FRIENDS, include_bytes!("../../assets/icons/friends.svg")),
    (PROFILE, include_bytes!("../../assets/icons/profile.svg")),
    (TEXT, include_bytes!("../../assets/icons/text.svg")),
    (VOICE, include_bytes!("../../assets/icons/voice.svg")),
    (ANNOUNCEMENT, include_bytes!("../../assets/icons/announcement.svg")),
    (FORUM, include_bytes!("../../assets/icons/forum.svg")),
    (STAGE, include_bytes!("../../assets/icons/stage.svg")),
    (CHEVRON, include_bytes!("../../assets/icons/chevron.svg")),
    (EMERALD, include_bytes!("../../assets/icons/emerald.svg")),
];

pub struct Icons;

impl AssetSource for Icons {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        Ok(ALL
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, _: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(ALL.iter().map(|(name, _)| (*name).into()).collect())
    }
}

pub fn icon(path: &'static str, size: f32, color: u32) -> Svg {
    svg().path(path).flex_none().size(px(size)).text_color(rgb(color))
}
