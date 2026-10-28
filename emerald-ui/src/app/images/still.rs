//! Still images whose decoded pixels leave memory with them. gpui decodes
//! an [`Image`] on its first draw and keeps the pixels, and their copy on
//! the GPU, until asked to forget them; [`Still`] asks when dropped.

use std::cell::RefCell;
use std::io::Cursor;
use std::sync::Arc;

use gpui::{Image, ImageFormat};

thread_local! {
    /// Images dropped since the last frame; only the UI thread makes them.
    static RELEASED: RefCell<Vec<Arc<Image>>> = const { RefCell::new(Vec::new()) };
}

/// An image the window may draw, freed from gpui's caches at the frame
/// after it is dropped (see [`super::Images::release`]).
pub struct Still(Arc<Image>);

impl Still {
    pub fn new(format: ImageFormat, bytes: Vec<u8>) -> Self {
        Self(Arc::new(Image::from_bytes(format, bytes)))
    }

    /// The image to hand to `img`.
    pub fn image(&self) -> Arc<Image> {
        self.0.clone()
    }
}

impl Drop for Still {
    fn drop(&mut self) {
        RELEASED.with(|released| released.borrow_mut().push(self.0.clone()));
    }
}

/// The images dropped since the last call.
pub(super) fn take_released() -> Vec<Arc<Image>> {
    RELEASED.with(|released| std::mem::take(&mut *released.borrow_mut()))
}

/// Bytes `bytes` take once gpui decodes them: four per pixel, and every
/// frame of a GIF (gpui keeps only the first of a WebP). Read from the
/// headers alone.
pub(super) fn decoded_size(bytes: &[u8], format: ImageFormat) -> usize {
    let format = match format {
        ImageFormat::Gif => return gif_size(bytes).unwrap_or(bytes.len()),
        ImageFormat::Png => image::ImageFormat::Png,
        ImageFormat::Jpeg => image::ImageFormat::Jpeg,
        ImageFormat::Webp => image::ImageFormat::WebP,
        _ => return bytes.len(),
    };
    // PNG and JPEG readers are there through gpui, which decodes both.
    image::ImageReader::with_format(Cursor::new(bytes), format)
        .into_dimensions()
        .map_or(bytes.len(), |(width, height)| width as usize * height as usize * 4)
}

/// A GIF's canvas times its frame count, walking its blocks without
/// decoding them.
fn gif_size(bytes: &[u8]) -> Option<usize> {
    let width = u16::from_le_bytes([*bytes.get(6)?, *bytes.get(7)?]) as usize;
    let height = u16::from_le_bytes([*bytes.get(8)?, *bytes.get(9)?]) as usize;
    let mut at = 13 + color_table(*bytes.get(10)?);
    let mut frames = 0;
    loop {
        match bytes.get(at) {
            // Extension: label, then data blocks.
            Some(0x21) => at = skip_blocks(bytes, at + 2)?,
            // Image: descriptor, color table, LZW code size, data blocks.
            Some(0x2c) => {
                frames += 1;
                at += 10 + color_table(*bytes.get(at + 9)?);
                at = skip_blocks(bytes, at + 1)?;
            }
            // Trailer, or a truncated file.
            _ => break,
        }
    }
    Some(width * height * 4 * frames.max(1))
}

/// Bytes of the color table a GIF's packed flags announce.
fn color_table(flags: u8) -> usize {
    if flags & 0x80 == 0 { 0 } else { 3 << ((flags & 7) + 1) }
}

/// Where the data blocks starting at `at` end.
fn skip_blocks(bytes: &[u8], mut at: usize) -> Option<usize> {
    loop {
        let len = *bytes.get(at)? as usize;
        at += 1 + len;
        if len == 0 {
            return Some(at);
        }
    }
}

#[cfg(test)]
mod tests {
    use image::codecs::gif::GifEncoder;
    use image::{Delay, Frame, Rgba, RgbaImage};

    use super::*;

    #[test]
    fn every_gif_frame_counts() {
        let mut bytes = Vec::new();
        {
            let mut encoder = GifEncoder::new(&mut bytes);
            let frames = (0..3).map(|n| {
                let image = RgbaImage::from_pixel(10, 20, Rgba([n * 80, 0, 0, 255]));
                Frame::from_parts(image, 0, 0, Delay::from_numer_denom_ms(50, 1))
            });
            encoder.encode_frames(frames).unwrap();
        }
        assert_eq!(decoded_size(&bytes, ImageFormat::Gif), 10 * 20 * 4 * 3);
    }

    #[test]
    fn png_size_comes_from_its_header() {
        let mut bytes = Vec::new();
        RgbaImage::new(30, 7).write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png).unwrap();
        assert_eq!(decoded_size(&bytes, ImageFormat::Png), 30 * 7 * 4);
    }

    #[test]
    fn dropped_images_wait_to_be_released() {
        drop(Still::new(ImageFormat::Png, vec![1]));
        assert_eq!(take_released().len(), 1);
        assert!(take_released().is_empty());
    }
}
