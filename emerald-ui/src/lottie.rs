//! Lottie animations, the format of Discord's own animated stickers (Wumpus
//! waving, …). velato reads the JSON and walks the shapes of a frame;
//! vello_cpu fills them into pixels. Frames are drawn as they are shown
//! rather than all up front: a 320px sticker is 400 KB a frame, and a few
//! seconds of them would hold tens of megabytes per sticker.
//!
//! Drawing happens on one background thread, a frame ahead of the one on
//! screen: most frames take under a millisecond, but some (big masks and
//! blends) take ten times that, which on the UI thread would stall the
//! whole window once per loop.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Duration;

use gpui::RenderImage;
use image::{Frame, RgbaImage};
use velato::model::{ImageAsset, fixed};
use velato::{Composition, RenderSink, Renderer};
use vello_cpu::kurbo::{Affine, BezPath, Shape};
use vello_cpu::peniko::{BlendMode, Brush};
use vello_cpu::{Pixmap, RenderContext, Resources};

/// Stickers play at most this fast; their usual 60 fps doubles the work for
/// no visible gain at sticker size.
pub const MAX_FPS: f64 = 30.;
/// Curve flattening tolerance, in output pixels.
const TOLERANCE: f64 = 0.1;
/// Frames are sampled this far past whole numbers. velato divides by zero
/// on a frame that falls exactly on a property's last keyframe, and
/// keyframes sit on whole frames: the shape would turn to NaN and vanish
/// for that frame, a visible blink.
const OFF_KEYFRAME: f64 = 1e-3;

/// One animation and the frames drawn for it.
pub struct Player {
    shared: Arc<Shared>,
}

struct Shared {
    composition: Composition,
    /// Square output size in pixels.
    size: u16,
    /// Only one frame of an animation is drawn at a time.
    drawing: Mutex<Drawing>,
    frames: Mutex<Frames>,
}

struct Drawing {
    renderer: Renderer,
    context: RenderContext,
    resources: Resources,
    pixmap: Pixmap,
}

#[derive(Default)]
struct Frames {
    /// Drawn frames by tick (index at the played rate), oldest first: the
    /// one on screen and the next.
    ready: Vec<(u64, Arc<RenderImage>)>,
    /// A frame is being drawn; no other is asked for meanwhile, so a slow
    /// machine skips frames instead of queueing them.
    drawing: bool,
    /// Drawing panicked once: the frame on screen stays, none are asked for.
    broken: bool,
    /// Frames replaced since the last [`Player::take_retired`], which still
    /// sit in the GPU atlas until dropped.
    retired: Vec<Arc<RenderImage>>,
}

/// The thread that draws frames ahead for every player.
fn worker() -> &'static Sender<(Arc<Shared>, u64)> {
    static WORKER: OnceLock<Sender<(Arc<Shared>, u64)>> = OnceLock::new();
    WORKER.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<(Arc<Shared>, u64)>();
        std::thread::Builder::new()
            .name("emerald-lottie".into())
            .spawn(move || {
                for (shared, tick) in rx {
                    let image = shared.draw(tick);
                    let mut frames = shared.lock_frames();
                    frames.drawing = false;
                    match image {
                        Some(image) if frames.ready.last().is_none_or(|(last, _)| *last < tick) => {
                            frames.ready.push((tick, image));
                        }
                        Some(_) => {}
                        None => frames.broken = true,
                    }
                }
            })
            .expect("spawn lottie thread");
        tx
    })
}

impl Player {
    /// Reads a Lottie file to play it at `size` pixels square; `None` when
    /// it is not one, or has no frames to show.
    #[hotpath::measure]
    pub fn new(bytes: &[u8], size: u16) -> Option<Self> {
        let composition = catch_unwind(|| Composition::from_slice(bytes))
            .map_err(|_| tracing::warn!("lottie: velato cannot read this animation"))
            .ok()?
            .map_err(|e| tracing::warn!("lottie: {e}"))
            .ok()?;
        let length = composition.frames.end - composition.frames.start;
        if !(length > 0. && composition.frame_rate > 0. && composition.width > 0 && composition.height > 0) {
            return None;
        }
        Some(Self {
            shared: Arc::new(Shared {
                composition,
                size,
                drawing: Mutex::new(Drawing {
                    renderer: Renderer::new(),
                    context: RenderContext::new(size, size),
                    resources: Resources::new(),
                    pixmap: Pixmap::new(size, size),
                }),
                frames: Mutex::default(),
            }),
        })
    }

    /// The tick (frame index at the played rate) `elapsed` falls in.
    pub fn tick(elapsed: Duration) -> u64 {
        (elapsed.as_secs_f64() * MAX_FPS) as u64
    }

    /// When the tick after the one `elapsed` falls in begins.
    pub fn next_tick(elapsed: Duration) -> Duration {
        Duration::from_secs_f64((Self::tick(elapsed) + 1) as f64 / MAX_FPS)
    }

    /// The frame to show `elapsed` into the animation's loop: the newest
    /// drawn one not ahead of it. The very first is drawn on the spot;
    /// after that the next one is always being drawn in the background.
    pub fn frame(&self, elapsed: Duration) -> Option<Arc<RenderImage>> {
        let shared = &self.shared;
        let tick = Self::tick(elapsed);
        let mut frames = shared.lock_frames();
        if frames.ready.is_empty() && !frames.drawing && !frames.broken {
            drop(frames);
            let image = shared.draw(tick);
            frames = shared.lock_frames();
            match image {
                Some(image) => frames.ready.push((tick, image)),
                None => frames.broken = true,
            }
        }
        // Everything older than the frame to show is done with.
        let shown = frames.ready.iter().rposition(|(t, _)| *t <= tick).unwrap_or(0);
        let done: Vec<_> = frames.ready.drain(..shown).map(|(_, image)| image).collect();
        frames.retired.extend(done);
        let image = frames.ready.first()?.1.clone();
        // On time, the next frame; behind (a slow machine, a pause), this one.
        let newest = frames.ready.last().map_or(0, |(last, _)| *last);
        let wanted = if newest < tick { tick } else { tick + 1 };
        if newest < wanted && !frames.drawing && !frames.broken {
            frames.drawing = true;
            worker().send((shared.clone(), wanted)).ok();
        }
        Some(image)
    }

    /// Frames no longer shown, for the caller to free.
    pub fn take_retired(&self) -> Vec<Arc<RenderImage>> {
        std::mem::take(&mut self.shared.lock_frames().retired)
    }

    /// Every frame this player still holds, when it goes away.
    pub fn into_images(self) -> Vec<Arc<RenderImage>> {
        let mut frames = self.shared.lock_frames();
        let mut images = std::mem::take(&mut frames.retired);
        images.extend(frames.ready.drain(..).map(|(_, image)| image));
        images
    }
}

impl Shared {
    fn lock_frames(&self) -> std::sync::MutexGuard<'_, Frames> {
        self.frames.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Draws the frame for `tick`, scaled into the square, as straight-alpha
    /// BGRA, which is what gpui uploads; `None` if velato panicked on it.
    fn draw(&self, tick: u64) -> Option<Arc<RenderImage>> {
        catch_unwind(AssertUnwindSafe(|| self.draw_unguarded(tick)))
            .map_err(|_| tracing::warn!("lottie: velato failed on frame {tick}"))
            .ok()
    }

    fn draw_unguarded(&self, tick: u64) -> Arc<RenderImage> {
        let composition = &self.composition;
        let length = composition.frames.end - composition.frames.start;
        let frame = composition.frames.start + (tick as f64 * composition.frame_rate / MAX_FPS) % length + OFF_KEYFRAME;
        let size = f64::from(self.size);
        let scale = size / composition.width.max(composition.height) as f64;
        let offset = (
            (size - composition.width as f64 * scale) / 2.,
            (size - composition.height as f64 * scale) / 2.,
        );
        let mut drawing = self.drawing.lock().unwrap_or_else(PoisonError::into_inner);
        let Drawing {
            renderer,
            context,
            resources,
            pixmap,
        } = &mut *drawing;
        context.reset();
        let transform = Affine::translate(offset) * Affine::scale(scale);
        renderer.append(composition, frame, transform, 1., &mut Canvas(context));
        context.flush();
        context.render(&mut *pixmap, resources);

        let mut bytes = Vec::with_capacity(usize::from(self.size).pow(2) * 4);
        for pixel in pixmap.data() {
            let unpremultiply = |c: u8| match pixel.a {
                0 => 0,
                a => ((u16::from(c) * 255 + u16::from(a) / 2) / u16::from(a)).min(255) as u8,
            };
            bytes.extend([unpremultiply(pixel.b), unpremultiply(pixel.g), unpremultiply(pixel.r), pixel.a]);
        }
        let image = RgbaImage::from_raw(u32::from(self.size), u32::from(self.size), bytes).expect("buffer matches size");
        Arc::new(RenderImage::new(vec![Frame::new(image)]))
    }
}

/// velato's drawing calls, on a vello_cpu context.
struct Canvas<'a>(&'a mut RenderContext);

fn path(shape: &impl Shape) -> BezPath {
    shape.to_path(TOLERANCE)
}

impl RenderSink for Canvas<'_> {
    fn push_layer(&mut self, blend: impl Into<BlendMode>, alpha: f32, transform: Affine, shape: &impl Shape) {
        self.0.set_transform(transform);
        self.0.push_layer(Some(&path(shape)), Some(blend.into()), Some(alpha), None, None);
    }

    fn push_clip_layer(&mut self, transform: Affine, shape: &impl Shape) {
        self.0.set_transform(transform);
        self.0.push_clip_layer(&path(shape));
    }

    fn pop_layer(&mut self) {
        self.0.pop_layer();
    }

    fn draw(&mut self, stroke: Option<&fixed::Stroke>, transform: Affine, brush: &fixed::Brush, shape: &impl Shape) {
        match brush {
            Brush::Solid(color) => self.0.set_paint(*color),
            Brush::Gradient(gradient) => self.0.set_paint(gradient.clone()),
            // Discord's stickers carry no bitmaps.
            Brush::Image(_) => return,
        }
        self.0.set_transform(transform);
        match stroke {
            Some(stroke) => {
                self.0.set_stroke(stroke.clone());
                self.0.stroke_path(&path(shape));
            }
            None => self.0.fill_path(&path(shape)),
        }
    }

    fn draw_image(&mut self, _: &ImageAsset, _: Affine, _: f64) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A red square that slides right over two seconds.
    const SLIDE: &str = r#"{
        "v": "5.7.0", "fr": 30, "ip": 0, "op": 60, "w": 100, "h": 100,
        "layers": [{
            "ty": 4, "ind": 1, "ip": 0, "op": 60, "st": 0,
            "ks": {
                "o": { "a": 0, "k": 100 },
                "r": { "a": 0, "k": 0 },
                "p": { "a": 1, "k": [
                    { "t": 0, "s": [25, 50, 0], "i": { "x": [1], "y": [1] }, "o": { "x": [0], "y": [0] } },
                    { "t": 60, "s": [75, 50, 0] }
                ] },
                "a": { "a": 0, "k": [0, 0, 0] },
                "s": { "a": 0, "k": [100, 100, 100] }
            },
            "shapes": [
                { "ty": "rc", "p": { "a": 0, "k": [0, 0] }, "s": { "a": 0, "k": [20, 20] }, "r": { "a": 0, "k": 0 } },
                { "ty": "fl", "c": { "a": 0, "k": [1, 0, 0, 1] }, "o": { "a": 0, "k": 100 } }
            ]
        }]
    }"#;

    /// Blue, green, red, alpha at `(x, y)` of a frame.
    fn pixel(image: &RenderImage, x: usize, y: usize) -> [u8; 4] {
        let bytes = image.as_bytes(0).unwrap();
        let at = (y * 100 + x) * 4;
        bytes[at..at + 4].try_into().unwrap()
    }

    /// Lets the background thread finish the frame it was asked for.
    fn settle() {
        std::thread::sleep(Duration::from_millis(100));
    }

    #[test]
    fn frames_follow_the_clock() {
        let player = Player::new(SLIDE.as_bytes(), 100).unwrap();
        // The first frame is there at once.
        let start = player.frame(Duration::ZERO).unwrap();
        assert_eq!(pixel(&start, 25, 50), [0, 0, 255, 255]);
        assert_eq!(pixel(&start, 75, 50)[3], 0);
        settle();
        // The next one was drawn ahead and shows when its time comes.
        let next = player.frame(Duration::from_millis(40)).unwrap();
        assert!(!Arc::ptr_eq(&start, &next));
        assert_eq!(player.take_retired().len(), 1);
        settle();
        // Far behind: a stale frame stays up while the due one is drawn.
        let late = player.frame(Duration::from_millis(1990)).unwrap();
        assert_eq!(pixel(&late, 75, 50)[3], 0);
        settle();
        let end = player.frame(Duration::from_millis(1990)).unwrap();
        assert_eq!(pixel(&end, 75, 50), [0, 0, 255, 255]);
        settle();
        // Loops back to the start.
        assert_eq!(pixel(&player.frame(Duration::from_secs(2)).unwrap(), 25, 50), [0, 0, 255, 255]);
    }

    /// A path whose morph ends on a whole frame stays drawn on that frame
    /// (velato alone gives NaN there).
    #[test]
    fn last_keyframe_does_not_blink() {
        let square = |x0: u32, x1: u32| {
            format!(
                r#"{{ "c": true, "i": [[0,0],[0,0],[0,0],[0,0]], "o": [[0,0],[0,0],[0,0],[0,0]],
                    "v": [[{x0},40],[{x1},40],[{x1},60],[{x0},60]] }}"#
            )
        };
        // Grows from the left half to the full width over frames 0–30.
        let morph = format!(
            r#"{{
                "v": "5.7.0", "fr": 30, "ip": 0, "op": 60, "w": 100, "h": 100,
                "layers": [{{
                    "ty": 4, "ind": 1, "ip": 0, "op": 60, "st": 0,
                    "ks": {{
                        "o": {{ "a": 0, "k": 100 }}, "r": {{ "a": 0, "k": 0 }}, "p": {{ "a": 0, "k": [0, 0, 0] }},
                        "a": {{ "a": 0, "k": [0, 0, 0] }}, "s": {{ "a": 0, "k": [100, 100, 100] }}
                    }},
                    "shapes": [
                        {{ "ty": "sh", "ks": {{ "a": 1, "k": [
                            {{ "t": 0, "s": [{}], "i": {{ "x": 1, "y": 1 }}, "o": {{ "x": 0, "y": 0 }} }},
                            {{ "t": 30, "s": [{}] }}
                        ] }} }},
                        {{ "ty": "fl", "c": {{ "a": 0, "k": [1, 0, 0, 1] }}, "o": {{ "a": 0, "k": 100 }} }}
                    ]
                }}]
            }}"#,
            square(10, 50),
            square(10, 90),
        );
        let player = Player::new(morph.as_bytes(), 100).unwrap();
        for tick in [29, 30, 31] {
            assert_eq!(pixel(&player.shared.draw(tick).unwrap(), 20, 50), [0, 0, 255, 255], "tick {tick}");
        }
    }

    #[test]
    fn rejects_what_is_not_lottie() {
        assert!(Player::new(b"\x89PNG", 100).is_none());
        // Valid Lottie velato has no code for (a transform without rotation):
        // refused instead of a crash.
        let unrotated = SLIDE.replacen(r#""r": { "a": 0, "k": 0 },"#, "", 1);
        assert!(Player::new(unrotated.as_bytes(), 100).is_none());
        assert!(Player::new(br#"{"v":"5","fr":30,"ip":0,"op":0,"w":10,"h":10,"layers":[]}"#, 100).is_none());
    }
}



