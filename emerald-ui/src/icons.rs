//! Monochrome SVG icons bundled into the binary. gpui paints them with the
//! svg element's own text color (black when unset); it is not inherited.

use std::borrow::Cow;

use gpui::{AssetSource, SharedString, Svg, prelude::*, rgb, svg};
use crate::zoom::zoomed;

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
pub const REPLY: &str = "icons/reply.svg";
pub const EDIT: &str = "icons/edit.svg";
pub const MORE: &str = "icons/more.svg";
pub const PIN: &str = "icons/pin.svg";
pub const TRASH: &str = "icons/trash.svg";
pub const COPY: &str = "icons/copy.svg";
pub const REACTION: &str = "icons/reaction.svg";
pub const ATTACH: &str = "icons/attach.svg";
pub const CLOSE: &str = "icons/close.svg";
pub const EYE: &str = "icons/eye.svg";
pub const THREAD: &str = "icons/thread.svg";
pub const SETTINGS: &str = "icons/settings.svg";
pub const PLAY: &str = "icons/play.svg";
pub const FILE: &str = "icons/file.svg";
pub const FORWARD: &str = "icons/forward.svg";
pub const SEARCH: &str = "icons/search.svg";
pub const INBOX: &str = "icons/inbox.svg";
pub const FOLDER: &str = "icons/folder.svg";
pub const CHECK: &str = "icons/check.svg";
pub const CHEVRON_RIGHT: &str = "icons/chevron_right.svg";
pub const USER_ADD: &str = "icons/user_add.svg";
pub const PLUS: &str = "icons/plus.svg";
pub const MICROPHONE: &str = "icons/microphone.svg";
pub const MICROPHONE_OFF: &str = "icons/microphone_off.svg";
pub const HEADPHONES: &str = "icons/headphones.svg";
pub const HEADPHONES_OFF: &str = "icons/headphones_off.svg";
pub const HANG_UP: &str = "icons/hang_up.svg";
pub const PHONE: &str = "icons/phone.svg";
/// End-to-end encrypted call.
pub const SHIELD: &str = "icons/shield.svg";

const ALL: [(&str, &[u8]); 38] = [
    (FRIENDS, include_bytes!("../assets/icons/friends.svg")),
    (PROFILE, include_bytes!("../assets/icons/profile.svg")),
    (TEXT, include_bytes!("../assets/icons/text.svg")),
    (VOICE, include_bytes!("../assets/icons/voice.svg")),
    (ANNOUNCEMENT, include_bytes!("../assets/icons/announcement.svg")),
    (FORUM, include_bytes!("../assets/icons/forum.svg")),
    (STAGE, include_bytes!("../assets/icons/stage.svg")),
    (CHEVRON, include_bytes!("../assets/icons/chevron.svg")),
    (EMERALD, include_bytes!("../assets/icons/emerald.svg")),
    (REPLY, include_bytes!("../assets/icons/reply.svg")),
    (EDIT, include_bytes!("../assets/icons/edit.svg")),
    (MORE, include_bytes!("../assets/icons/more.svg")),
    (PIN, include_bytes!("../assets/icons/pin.svg")),
    (TRASH, include_bytes!("../assets/icons/trash.svg")),
    (COPY, include_bytes!("../assets/icons/copy.svg")),
    (REACTION, include_bytes!("../assets/icons/reaction.svg")),
    (ATTACH, include_bytes!("../assets/icons/attach.svg")),
    (CLOSE, include_bytes!("../assets/icons/close.svg")),
    (EYE, include_bytes!("../assets/icons/eye.svg")),
    (THREAD, include_bytes!("../assets/icons/thread.svg")),
    (SETTINGS, include_bytes!("../assets/icons/settings.svg")),
    (PLAY, include_bytes!("../assets/icons/play.svg")),
    (FILE, include_bytes!("../assets/icons/file.svg")),
    (FORWARD, include_bytes!("../assets/icons/forward.svg")),
    (SEARCH, include_bytes!("../assets/icons/search.svg")),
    (INBOX, include_bytes!("../assets/icons/inbox.svg")),
    (FOLDER, include_bytes!("../assets/icons/folder.svg")),
    (CHECK, include_bytes!("../assets/icons/check.svg")),
    (CHEVRON_RIGHT, include_bytes!("../assets/icons/chevron_right.svg")),
    (USER_ADD, include_bytes!("../assets/icons/user_add.svg")),
    (PLUS, include_bytes!("../assets/icons/plus.svg")),
    (MICROPHONE, include_bytes!("../assets/icons/microphone.svg")),
    (MICROPHONE_OFF, include_bytes!("../assets/icons/microphone_off.svg")),
    (HEADPHONES, include_bytes!("../assets/icons/headphones.svg")),
    (HEADPHONES_OFF, include_bytes!("../assets/icons/headphones_off.svg")),
    (HANG_UP, include_bytes!("../assets/icons/hang_up.svg")),
    (PHONE, include_bytes!("../assets/icons/phone.svg")),
    (SHIELD, include_bytes!("../assets/icons/shield.svg")),
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
    svg().path(path).flex_none().size(zoomed(size)).text_color(rgb(color))
}
