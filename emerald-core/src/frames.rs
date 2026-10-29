//! Animated GIF and WebP decoded into frames for the interface, off its
//! thread. gpui decodes every GIF frame itself but only the first of a
//! WebP, and GIFs and WebPs from Klipy are what the GIF picker sends now.
//! Frames are scaled down to the size they are drawn at, and an animation
//! stops growing past a memory cap.

use std::io::Cursor;

use image::codecs::gif::GifDecoder;
use image::codecs::webp::WebPDecoder;
use fast_image_resize::images::{Image, ImageRef};
use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};
use image::{AnimationDecoder, DynamicImage, Frame, RgbaImage};

/// Decoded bytes one animation may hold; frames past it are dropped, so a
/// long GIF loops over its start.
const MAX_BYTES: usize = 32 * 1024 * 1024;

/// The frames of a GIF or WebP at most `max_width` by `max_height`, in the
/// BGRA order gpui draws; a still image comes back as one frame. `None`
/// for other formats and broken files.
#[hotpath::measure]
pub fn decode(bytes: &[u8], max_width: u32, max_height: u32) -> Option<Vec<Frame>> {
    let frames: Box<dyn Iterator<Item = image::ImageResult<Frame>>> = match bytes {
        [b'G', b'I', b'F', b'8', ..] => Box::new(GifDecoder::new(Cursor::new(bytes)).ok()?.into_frames()),
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => {
            let decoder = WebPDecoder::new(Cursor::new(bytes)).ok()?;
            if decoder.has_animation() {
                Box::new(decoder.into_frames())
            } else {
                let still = DynamicImage::from_decoder(decoder).ok()?.into_rgba8();
                Box::new(std::iter::once(Ok(Frame::new(still))))
            }
        }
        _ => return None,
    };
    let mut out = Vec::new();
    let mut total = 0;
    // One resizer for every frame: it keeps its scratch buffers.
    let mut resizer = Resizer::new();
    for frame in frames {
        let frame = frame.ok()?;
        let delay = frame.delay();
        let mut buffer = fit(&mut resizer, frame.into_buffer(), max_width, max_height);
        total += buffer.as_raw().len();
        if total > MAX_BYTES && !out.is_empty() {
            break;
        }
        // RGBA to the BGRA gpui's atlas takes.
        for pixel in buffer.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
        out.push(Frame::from_parts(buffer, 0, 0, delay));
    }
    (!out.is_empty()).then_some(out)
}

/// `buffer` scaled down to fit the box, never up, with a bilinear filter
/// (SIMD where the CPU has it) and transparent edges kept clean.
fn fit(resizer: &mut Resizer, buffer: RgbaImage, max_width: u32, max_height: u32) -> RgbaImage {
    let (width, height) = buffer.dimensions();
    let scale = (max_width as f32 / width as f32).min(max_height as f32 / height as f32);
    if scale >= 1. || width == 0 || height == 0 {
        return buffer;
    }
    let (w, h) = (((width as f32 * scale).round() as u32).max(1), ((height as f32 * scale).round() as u32).max(1));
    let Ok(source) = ImageRef::new(width, height, buffer.as_raw(), PixelType::U8x4) else {
        return buffer;
    };
    let mut target = Image::new(w, h, PixelType::U8x4);
    let options = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Bilinear));
    if resizer.resize(&source, &mut target, &options).is_err() {
        return buffer;
    }
    RgbaImage::from_raw(w, h, target.into_vec()).unwrap_or(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::codecs::gif::GifEncoder;
    use image::{Delay, Rgba};

    fn gif(frames: usize, side: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = GifEncoder::new(&mut bytes);
            let frames = (0..frames).map(|n| {
                let image = RgbaImage::from_pixel(side, side, Rgba([n as u8 * 40, 0, 200, 255]));
                Frame::from_parts(image, 0, 0, Delay::from_numer_denom_ms(50, 1))
            });
            encoder.encode_frames(frames).unwrap();
        }
        bytes
    }

    #[test]
    fn gifs_come_out_scaled_in_bgra() {
        let frames = decode(&gif(3, 40), 20, 20).unwrap();
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].buffer().dimensions(), (20, 20));
        // Blue went first: BGRA.
        assert_eq!(frames[0].buffer().get_pixel(5, 5)[0], 200);
    }

    #[test]
    fn other_formats_are_refused() {
        assert!(decode(b"\x89PNG....", 10, 10).is_none());
        assert!(decode(b"GIF89a broken", 10, 10).is_none());
    }
}
