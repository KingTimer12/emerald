//! Monochrome SVG icons bundled into the binary. gpui paints them with the
//! svg element's own text color (black when unset); it is not inherited.

use std::borrow::Cow;

use gpui::{AssetSource, SharedString, Svg, prelude::*, px, rgb, svg};

pub const FRIENDS: &str = "icons/friends.svg";

pub struct Icons;

impl AssetSource for Icons {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        let bytes: &'static [u8] = match path {
            FRIENDS => include_bytes!("../../assets/icons/friends.svg"),
            _ => return Ok(None),
        };
        Ok(Some(Cow::Borrowed(bytes)))
    }

    fn list(&self, _: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(vec![FRIENDS.into()])
    }
}

pub fn icon(path: &'static str, size: f32, color: u32) -> Svg {
    svg().path(path).flex_none().size(px(size)).text_color(rgb(color))
}
