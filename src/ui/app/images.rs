//! Images by CDN URL (avatars, icons, attachments, stickers), each
//! requested once and decoded when it arrives; Lottie stickers become
//! players that draw their frames as they are shown.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::Arc;

use gpui::{Image, ImageFormat, RenderImage, SharedString};

use super::super::lottie::Player;
use super::outbox::Outbox;
use crate::discord::Command;

/// Images kept decoded; scrolling a huge member list would otherwise grow
/// the cache without bound. Evicted ones come back from the disk cache.
const MAX_IMAGES: usize = 1500;
/// Lottie players kept; each holds its parsed animation and a frame buffer.
const MAX_ANIMATIONS: usize = 16;
/// Lottie frames are drawn at this size, twice the shown 160px for Retina.
pub const ANIMATION_PIXELS: u16 = 320;

#[derive(Default)]
pub struct Images {
    /// `None` while downloading or after a failure, so each URL is requested
    /// once. Interior mutability lets render request what it is about to draw.
    slots: RefCell<HashMap<SharedString, Option<Arc<Image>>>>,
    /// URLs in the order they were first requested, oldest first.
    order: RefCell<VecDeque<SharedString>>,
    /// Lottie stickers, kept like `slots`.
    animations: RefCell<HashMap<SharedString, Option<Rc<Player>>>>,
    animation_order: RefCell<VecDeque<SharedString>>,
    /// Frames of evicted players, still to be freed from the GPU.
    evicted: RefCell<Vec<Arc<RenderImage>>>,
}

impl Images {
    /// The image at `url` if downloaded; otherwise starts the download (once).
    pub fn get(&self, url: &str, outbox: &dyn Outbox) -> Option<Arc<Image>> {
        let mut slots = self.slots.borrow_mut();
        if let Some(slot) = slots.get(url) {
            return slot.clone();
        }
        let url: SharedString = url.to_owned().into();
        let mut order = self.order.borrow_mut();
        while order.len() >= MAX_IMAGES {
            if let Some(oldest) = order.pop_front() {
                slots.remove(&oldest);
            }
        }
        order.push_back(url.clone());
        slots.insert(url.clone(), None);
        outbox.post(Command::FetchAvatar { url: url.to_string() });
        None
    }

    /// The Lottie player for `url` once downloaded; otherwise starts the
    /// download (once).
    pub fn animation(&self, url: &str, outbox: &dyn Outbox) -> Option<Rc<Player>> {
        let mut slots = self.animations.borrow_mut();
        if let Some(slot) = slots.get(url) {
            return slot.clone();
        }
        let url: SharedString = url.to_owned().into();
        let mut order = self.animation_order.borrow_mut();
        while order.len() >= MAX_ANIMATIONS {
            let Some(oldest) = order.pop_front() else { break };
            let player = slots.remove(&oldest).flatten().and_then(|p| Rc::try_unwrap(p).ok());
            if let Some(player) = player {
                self.evicted.borrow_mut().extend(player.into_images());
            }
        }
        order.push_back(url.clone());
        slots.insert(url.clone(), None);
        outbox.post(Command::FetchAvatar { url: url.to_string() });
        None
    }

    /// Frames no longer on screen, to be dropped from the GPU atlas.
    pub fn take_retired(&self) -> Vec<Arc<RenderImage>> {
        let mut retired = std::mem::take(&mut *self.evicted.borrow_mut());
        for player in self.animations.borrow().values().flatten() {
            retired.extend(player.take_retired());
        }
        retired
    }

    /// The download for `url` finished; `None` or undecodable bytes leave a
    /// permanent blank.
    pub fn store(&self, url: String, bytes: Option<Vec<u8>>) {
        if let Some(slot) = self.animations.borrow_mut().get_mut(url.as_str()) {
            *slot = bytes
                .and_then(|bytes| Player::new(&bytes, ANIMATION_PIXELS))
                .map(Rc::new);
            return;
        }
        let image = bytes.and_then(|bytes| {
            let format = image_format(&bytes)?;
            Some(Arc::new(Image::from_bytes(format, bytes)))
        });
        // Evicted while downloading: dropped rather than kept untracked.
        if let Some(slot) = self.slots.borrow_mut().get_mut(url.as_str()) {
            *slot = image;
        }
    }
}

/// Image format from the file's magic bytes; CDN URLs do not always say.
fn image_format(bytes: &[u8]) -> Option<ImageFormat> {
    match bytes {
        [0x89, b'P', b'N', b'G', ..] => Some(ImageFormat::Png),
        [0xff, 0xd8, 0xff, ..] => Some(ImageFormat::Jpeg),
        [b'G', b'I', b'F', b'8', ..] => Some(ImageFormat::Gif),
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => Some(ImageFormat::Webp),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    #[test]
    fn each_url_is_requested_once() {
        let images = Images::default();
        let outbox = RefCell::new(Vec::new());
        assert!(images.get("https://cdn/a.png", &outbox).is_none());
        assert!(images.get("https://cdn/a.png", &outbox).is_none());
        assert_eq!(outbox.borrow().len(), 1);
        images.store("https://cdn/a.png".into(), Some(b"\x89PNG....".to_vec()));
        assert!(images.get("https://cdn/a.png", &outbox).is_some());
    }

    #[test]
    fn oldest_images_are_evicted() {
        let images = Images::default();
        let outbox = RefCell::new(Vec::new());
        for n in 0..MAX_IMAGES + 1 {
            images.get(&format!("https://cdn/{n}.png"), &outbox);
        }
        assert_eq!(images.slots.borrow().len(), MAX_IMAGES);
        assert!(!images.slots.borrow().contains_key("https://cdn/0.png"));
    }

    #[test]
    fn formats_from_magic_bytes() {
        assert_eq!(image_format(b"GIF89a"), Some(ImageFormat::Gif));
        assert_eq!(image_format(b"RIFF\0\0\0\0WEBPVP8"), Some(ImageFormat::Webp));
        assert_eq!(image_format(b"<html>"), None);
    }
}
