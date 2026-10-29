//! Images by CDN URL (avatars, icons, attachments, stickers), each
//! requested once and decoded when it arrives; Lottie stickers become
//! players that draw their frames as they are shown.

mod budget;
mod still;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::Arc;

use gpui::{App, Image, ImageFormat, RenderImage, SharedString, Window};

use super::super::lottie::Player;
use super::outbox::Outbox;
use budget::Budget;
use emerald_discord::Command;
pub use still::Still;

/// Images kept; scrolling a huge member list would otherwise grow the
/// cache without bound. Evicted ones come back from the disk cache.
const MAX_IMAGES: usize = 1500;
/// Bytes the kept images take once decoded, which is what they cost: a
/// 128px avatar is 64 KB of pixels against a few KB of PNG.
const MAX_DECODED: usize = 48 * 1024 * 1024;
/// Lottie players kept for the chat; each holds its parsed animation and a
/// frame buffer.
const MAX_ANIMATIONS: usize = 16;
/// Chat stickers are drawn at this size, twice the shown 160px for Retina.
pub const ANIMATION_PIXELS: u16 = 320;
/// Players kept for the sticker picker, more than a screenful of its rows.
const MAX_PICKER_ANIMATIONS: usize = 48;
/// Picker stickers are drawn at twice the picker's 80px.
const PICKER_PIXELS: u16 = 160;
/// Animated GIFs and WebPs kept decoded, all frames each: a screenful.
const MAX_ANIMATED: usize = 8;
/// Bytes their frames may take together; one alone may hold up to 32 MB
/// (see `emerald_core::frames`).
const MAX_ANIMATED_BYTES: usize = 64 * 1024 * 1024;
/// Animations this many pixels or smaller (emoji, avatars) count apart:
/// a screen holds dozens of them, each a fraction of a GIF embed.
const SMALL_PIXELS: u32 = 160 * 160;
/// Small animations kept decoded, and the bytes their frames may take.
const MAX_SMALL_ANIMATED: usize = 512;
const MAX_SMALL_ANIMATED_BYTES: usize = 32 * 1024 * 1024;

pub struct Images {
    /// Still images. Interior mutability lets render request what it is
    /// about to draw.
    stills: RefCell<Budget<Still>>,
    /// Counts frames, to tell which images are on screen.
    frame: Cell<u64>,
    /// Lottie stickers in the chat.
    chat: Players<MAX_ANIMATIONS, ANIMATION_PIXELS>,
    /// Lottie stickers in the sticker picker, smaller and more of them.
    picker: Players<MAX_PICKER_ANIMATIONS, PICKER_PIXELS>,
    /// Frames of evicted players and animations, still to be freed from
    /// the GPU.
    evicted: RefCell<Vec<Arc<RenderImage>>>,
    /// Animated images by URL, decoded on the network thread.
    animations: RefCell<Budget<Arc<RenderImage>>>,
    /// Small ones (see [`SMALL_PIXELS`]).
    small_animations: RefCell<Budget<Arc<RenderImage>>>,
}

impl Default for Images {
    fn default() -> Self {
        Self {
            stills: RefCell::new(Budget::new(MAX_IMAGES, MAX_DECODED)),
            frame: Cell::default(),
            chat: Players::default(),
            picker: Players::default(),
            evicted: RefCell::default(),
            animations: RefCell::new(Budget::new(MAX_ANIMATED, MAX_ANIMATED_BYTES)),
            small_animations: RefCell::new(Budget::new(MAX_SMALL_ANIMATED, MAX_SMALL_ANIMATED_BYTES)),
        }
    }
}

/// Lottie players by URL, at most `MAX` of them, drawing `PIXELS` square.
/// Only players being shown ask for frames, so the ones scrolled away cost
/// memory but no time.
#[derive(Default)]
struct Players<const MAX: usize, const PIXELS: u16> {
    slots: RefCell<HashMap<SharedString, Option<Rc<Player>>>>,
    order: RefCell<VecDeque<SharedString>>,
}

impl<const MAX: usize, const PIXELS: u16> Players<MAX, PIXELS> {
    fn get(&self, url: &str, outbox: &dyn Outbox, evicted: &RefCell<Vec<Arc<RenderImage>>>) -> Option<Rc<Player>> {
        let mut slots = self.slots.borrow_mut();
        if let Some(slot) = slots.get(url) {
            return slot.clone();
        }
        let url: SharedString = url.to_owned().into();
        let mut order = self.order.borrow_mut();
        while order.len() >= MAX {
            let Some(oldest) = order.pop_front() else { break };
            let player = slots.remove(&oldest).flatten().and_then(|p| Rc::try_unwrap(p).ok());
            if let Some(player) = player {
                evicted.borrow_mut().extend(player.into_images());
            }
        }
        order.push_back(url.clone());
        slots.insert(url.clone(), None);
        outbox.post(Command::FetchAvatar { url: url.to_string() });
        None
    }

    /// The size to read `url`'s download at, if a player here waits for it.
    fn waiting(&self, url: &str) -> Option<u16> {
        self.slots.borrow().contains_key(url).then_some(PIXELS)
    }

    /// Puts the player read from `url`'s download in its slot, if it is
    /// still there; `None` leaves a permanent blank.
    fn fill(&self, url: &str, player: Option<Player>) {
        if let Some(slot) = self.slots.borrow_mut().get_mut(url) {
            *slot = player.map(Rc::new);
        }
    }

    fn take_retired(&self, into: &mut Vec<Arc<RenderImage>>) {
        for player in self.slots.borrow().values().flatten() {
            into.extend(player.take_retired());
        }
    }
}

impl Images {
    /// The image at `url` if downloaded; otherwise starts the download (once).
    pub fn get(&self, url: &str, outbox: &dyn Outbox) -> Option<Arc<Image>> {
        let mut stills = self.stills.borrow_mut();
        if let Some(slot) = stills.get(url, self.frame.get()) {
            return slot.map(Still::image);
        }
        // Evicted ones free their pixels at the next `release`.
        stills.ask(url.to_owned().into(), self.frame.get());
        outbox.post(Command::FetchAvatar { url: url.to_owned() });
        None
    }

    /// Frees what was evicted or dropped since the last frame, from memory
    /// and from the GPU, and starts a new frame. Called once per render.
    #[hotpath::measure]
    pub fn release(&self, window: &mut Window, cx: &mut App) {
        self.frame.set(self.frame.get() + 1);
        for image in self.take_retired() {
            cx.drop_image(image, Some(window));
        }
        for image in still::take_released() {
            // Only an image already decoded comes back here; one still
            // decoding is cancelled by `remove_asset`.
            if let Some(pixels) = image.clone().get_render_image(window, cx) {
                cx.drop_image(pixels, Some(window));
            }
            image.remove_asset(cx);
        }
    }

    /// The Lottie player for `url` in the chat once downloaded; otherwise
    /// starts the download (once).
    pub fn animation(&self, url: &str, outbox: &dyn Outbox) -> Option<Rc<Player>> {
        self.chat.get(url, outbox, &self.evicted)
    }

    /// The same for the sticker picker's smaller players.
    pub fn picker_animation(&self, url: &str, outbox: &dyn Outbox) -> Option<Rc<Player>> {
        self.picker.get(url, outbox, &self.evicted)
    }

    /// The animated GIF or WebP at `url`, its frames fit to `width` by
    /// `height` pixels, once decoded; otherwise starts the download (once).
    /// gpui's `img` plays it when given an element id.
    pub fn animated(&self, url: &str, width: u32, height: u32, outbox: &dyn Outbox) -> Option<Arc<RenderImage>> {
        let budget = if width * height <= SMALL_PIXELS { &self.small_animations } else { &self.animations };
        let mut animations = budget.borrow_mut();
        if let Some(slot) = animations.get(url, self.frame.get()) {
            return slot.cloned();
        }
        let evicted = animations.ask(url.to_owned().into(), self.frame.get());
        self.evicted.borrow_mut().extend(evicted);
        outbox.post(Command::FetchAnimation {
            url: url.to_owned(),
            width,
            height,
        });
        None
    }

    /// The frames for `url` arrived; `None` leaves a permanent blank, and
    /// the caller its still.
    #[hotpath::measure]
    pub fn store_animation(&self, url: String, frames: Option<Vec<image::Frame>>) {
        let Some(frames) = frames else { return };
        let bytes = frames.iter().map(|frame| frame.buffer().as_raw().len()).sum();
        let frames = Arc::new(RenderImage::new(frames));
        // Evicted while decoding: dropped rather than kept untracked.
        let budget = if self.small_animations.borrow().contains(&url) { &self.small_animations } else { &self.animations };
        let evicted = budget.borrow_mut().fill(&url, frames, bytes, self.frame.get());
        self.evicted.borrow_mut().extend(evicted);
    }

    /// The sizes Lottie players wait to be read at from `url`'s download,
    /// the chat's and the picker's: the same sticker may play in both.
    /// Both `None` for anything else, which goes to [`Self::store`].
    pub fn lottie_sizes(&self, url: &str) -> [Option<u16>; 2] {
        [self.chat.waiting(url), self.picker.waiting(url)]
    }

    /// The players read from `url`'s download, in the order of
    /// [`Self::lottie_sizes`]; `None` leaves a permanent blank.
    pub fn store_players(&self, url: &str, [chat, picker]: [Option<Player>; 2]) {
        self.chat.fill(url, chat);
        self.picker.fill(url, picker);
    }

    /// Frames no longer on screen, to be dropped from the GPU atlas.
    pub fn take_retired(&self) -> Vec<Arc<RenderImage>> {
        let mut retired = std::mem::take(&mut *self.evicted.borrow_mut());
        self.chat.take_retired(&mut retired);
        self.picker.take_retired(&mut retired);
        retired
    }

    /// The download for `url` finished; `None` or undecodable bytes leave a
    /// permanent blank.
    #[hotpath::measure]
    pub fn store(&self, url: String, bytes: Option<Vec<u8>>) {
        let Some(bytes) = bytes else { return };
        let Some(format) = image_format(&bytes) else {
            // The host only: paths can name channels and files.
            let host = url.split('/').nth(2).unwrap_or_default();
            tracing::warn!("image from {host} is in a format that cannot be drawn");
            return;
        };
        let decoded = still::decoded_size(&bytes, format);
        // gpui decodes it whole: one claiming more pixels than the whole
        // budget holds would take that much memory at once.
        if decoded > MAX_DECODED {
            let host = url.split('/').nth(2).unwrap_or_default();
            tracing::warn!("image from {host} is too large to draw");
            return;
        }
        // Evicted while downloading, or pushed out: their pixels go at the
        // next `release`.
        self.stills.borrow_mut().fill(&url, Still::new(format, bytes), decoded, self.frame.get());
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
    fn stickers_wait_for_their_players() {
        let images = Images::default();
        let outbox = RefCell::new(Vec::new());
        let url = "https://cdn/sticker.json";
        assert_eq!(images.lottie_sizes(url), [None, None]);
        assert!(images.animation(url, &outbox).is_none());
        assert_eq!(images.lottie_sizes(url), [Some(ANIMATION_PIXELS), None]);
        let lottie = br#"{"v":"5","fr":30,"ip":0,"op":30,"w":10,"h":10,"layers":[]}"#;
        images.store_players(url, [Player::new(lottie, ANIMATION_PIXELS), None]);
        assert!(images.animation(url, &outbox).is_some());
        assert_eq!(outbox.borrow().len(), 1);
    }

    #[test]
    fn oldest_images_are_evicted() {
        let images = Images::default();
        let outbox = RefCell::new(Vec::new());
        for n in 0..MAX_IMAGES {
            images.get(&format!("https://cdn/{n}.png"), &outbox);
        }
        images.frame.set(2);
        // Still on screen: kept, and the next oldest goes instead.
        images.get("https://cdn/0.png", &outbox);
        images.get("https://cdn/new.png", &outbox);
        assert_eq!(images.stills.borrow().len(), MAX_IMAGES);
        assert!(images.stills.borrow().contains("https://cdn/0.png"));
        assert!(!images.stills.borrow().contains("https://cdn/1.png"));
    }

    /// An empty GIF of `side` by `side` pixels.
    fn gif(side: u16) -> Vec<u8> {
        let mut bytes = b"GIF89a".to_vec();
        bytes.extend(side.to_le_bytes());
        bytes.extend(side.to_le_bytes());
        bytes.extend([0, 0, 0, 0x3b]);
        bytes
    }

    #[test]
    fn decoded_bytes_are_capped_but_not_on_screen() {
        let images = Images::default();
        let outbox = RefCell::new(Vec::new());
        // 3000² RGBA is 36 MB: two of them pass the cap together.
        images.get("https://cdn/big.gif", &outbox);
        images.store("https://cdn/big.gif".into(), Some(gif(3000)));
        assert!(images.stills.borrow().contains("https://cdn/big.gif"));
        images.frame.set(2);
        images.get("https://cdn/other.gif", &outbox);
        images.store("https://cdn/other.gif".into(), Some(gif(3000)));
        assert!(!images.stills.borrow().contains("https://cdn/big.gif"));
        assert_eq!(images.stills.borrow().bytes(), 3000 * 3000 * 4);
        // Its pixels wait to be freed by the window.
        assert_eq!(still::take_released().len(), 1);
    }

    #[test]
    fn images_larger_than_the_whole_budget_are_refused() {
        let images = Images::default();
        let outbox = RefCell::new(Vec::new());
        // 4096² RGBA is 64 MB, more than all stills may take together.
        images.get("https://cdn/huge.gif", &outbox);
        images.store("https://cdn/huge.gif".into(), Some(gif(4096)));
        assert_eq!(images.stills.borrow().bytes(), 0);
    }

    #[test]
    fn animations_are_asked_for_once_and_capped() {
        let images = Images::default();
        let outbox = RefCell::new(Vec::new());
        assert!(images.animated("https://x/a.webp", 400, 300, &outbox).is_none());
        assert!(images.animated("https://x/a.webp", 400, 300, &outbox).is_none());
        assert_eq!(outbox.borrow().len(), 1);
        let frame = image::Frame::new(image::RgbaImage::new(1, 1));
        images.store_animation("https://x/a.webp".into(), Some(vec![frame]));
        assert!(images.animated("https://x/a.webp", 400, 300, &outbox).is_some());
        images.frame.set(2);
        for n in 0..MAX_ANIMATED {
            images.animated(&format!("https://x/{n}.gif"), 400, 300, &outbox);
        }
        assert_eq!(images.animations.borrow().len(), MAX_ANIMATED);
        // The decoded one went out and waits to be freed.
        assert_eq!(images.take_retired().len(), 1);
    }

    #[test]
    fn small_animations_do_not_push_out_embeds() {
        let images = Images::default();
        let outbox = RefCell::new(Vec::new());
        images.animated("https://x/embed.webp", 400, 300, &outbox);
        images.frame.set(2);
        // A screenful of moving emoji, more than the embeds' cap.
        for n in 0..MAX_ANIMATED * 4 {
            images.animated(&format!("https://cdn/emojis/{n}.webp"), 48, 48, &outbox);
        }
        let frame = image::Frame::new(image::RgbaImage::new(48, 48));
        images.store_animation("https://cdn/emojis/0.webp".into(), Some(vec![frame]));
        assert!(images.animations.borrow().contains("https://x/embed.webp"));
        assert!(images.animated("https://cdn/emojis/0.webp", 48, 48, &outbox).is_some());
        assert_eq!(images.small_animations.borrow().len(), MAX_ANIMATED * 4);
    }

    #[test]
    fn formats_from_magic_bytes() {
        assert_eq!(image_format(b"GIF89a"), Some(ImageFormat::Gif));
        assert_eq!(image_format(b"RIFF\0\0\0\0WEBPVP8"), Some(ImageFormat::Webp));
        assert_eq!(image_format(b"<html>"), None);
    }
}
