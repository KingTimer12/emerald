//! Sound in and out for voice calls, in the one shape voice codecs want:
//! 20 ms frames of 48 kHz mono samples.
//!
//! ```no_run
//! use emerald_audio::{Audio, Choice};
//!
//! let (audio, mut microphone, mut speaker) = Audio::open(&Choice::default()).unwrap();
//! // Every 20 ms: a frame captured, a frame to play.
//! if let Ok(frame) = microphone.pop() {
//!     speaker.push(frame).ok();
//! }
//! audio.set_muted(true);
//! ```
//!
//! Devices run at their own rate and channel count; frames are converted on
//! the way in and out. The microphone is optional: when it cannot be opened
//! the speaker still plays, and [`Audio::microphone_failed`] says so.

mod denoise;
mod devices;
mod resample;
mod streams;

pub use denoise::Denoiser;
pub use devices::{Device, Devices, devices};
pub use streams::{Audio, Choice, Microphone, Speaker};

/// Samples per second in a frame.
pub const RATE: u32 = 48_000;
/// Samples in a frame: 20 ms at [`RATE`].
pub const FRAME: usize = 960;
/// 20 ms of mono sound, samples between -1 and 1.
pub type Frame = [f32; FRAME];

/// How loud a frame is, in decibels below full scale; silence is -100.
pub fn level(frame: &Frame) -> f32 {
    let power = frame.iter().filter(|s| s.is_finite()).map(|s| s * s).sum::<f32>() / FRAME as f32;
    if power <= 1e-10 { -100. } else { 10. * power.log10() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_of_silence_and_full_scale() {
        assert_eq!(level(&[0.; FRAME]), -100.);
        assert!(level(&[1.; FRAME]).abs() < 0.01);
        assert!((level(&[0.1; FRAME]) + 20.).abs() < 0.01);
    }
}
