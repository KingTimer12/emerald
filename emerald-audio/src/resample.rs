//! Rate conversion between a device and 48 kHz frames. Linear
//! interpolation: cheap, and good enough for speech from the rates devices
//! fall back to (44.1, 24 or 16 kHz) when they do not offer 48 kHz.

use crate::{FRAME, Frame, RATE};

/// Device samples in, whole frames out.
pub struct ToFrames {
    /// Device samples per output sample.
    step: f64,
    /// Where the next output sample falls, between `previous` and the next
    /// input sample.
    phase: f64,
    previous: Option<f32>,
    frame: Frame,
    filled: usize,
}

impl ToFrames {
    pub fn new(device_rate: u32) -> Self {
        Self {
            step: f64::from(device_rate) / f64::from(RATE),
            phase: 0.,
            previous: None,
            frame: [0.; FRAME],
            filled: 0,
        }
    }

    pub fn reset(&mut self) {
        self.phase = 0.;
        self.previous = None;
        self.filled = 0;
    }

    /// Takes one device sample; `done` gets each frame it completes.
    pub fn push(&mut self, sample: f32, mut done: impl FnMut(Frame)) {
        if let Some(previous) = self.previous {
            while self.phase < 1. {
                self.frame[self.filled] = previous + (sample - previous) * self.phase as f32;
                self.filled += 1;
                if self.filled == FRAME {
                    done(self.frame);
                    self.filled = 0;
                }
                self.phase += self.step;
            }
            self.phase -= 1.;
        }
        self.previous = Some(sample);
    }
}

/// Whole frames in, device samples out.
pub struct FromFrames {
    /// Frame samples per device sample.
    step: f64,
    phase: f64,
    previous: f32,
    next: f32,
    frame: Frame,
    read: usize,
}

impl FromFrames {
    pub fn new(device_rate: u32) -> Self {
        Self {
            step: f64::from(RATE) / f64::from(device_rate),
            phase: 1.,
            previous: 0.,
            next: 0.,
            frame: [0.; FRAME],
            read: FRAME,
        }
    }

    /// The next device sample; `fetch` hands over the next frame when the
    /// current one runs out, or `None` to play silence.
    pub fn sample(&mut self, mut fetch: impl FnMut() -> Option<Frame>) -> f32 {
        while self.phase >= 1. {
            if self.read == FRAME {
                self.frame = fetch().unwrap_or([0.; FRAME]);
                self.read = 0;
            }
            self.previous = self.next;
            self.next = self.frame[self.read];
            self.read += 1;
            self.phase -= 1.;
        }
        let value = self.previous + (self.next - self.previous) * self.phase as f32;
        self.phase += self.step;
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames_from(rate: u32, seconds: f64) -> usize {
        let mut converter = ToFrames::new(rate);
        let mut count = 0;
        for _ in 0..(f64::from(rate) * seconds) as usize {
            converter.push(0.5, |frame| {
                assert!(frame.iter().all(|s| (s - 0.5).abs() < 1e-6));
                count += 1;
            });
        }
        count
    }

    #[test]
    fn a_second_of_any_rate_makes_fifty_frames() {
        for rate in [16_000, 24_000, 44_100, 48_000, 96_000] {
            let frames = frames_from(rate, 1.);
            assert!((49..=50).contains(&frames), "{rate} Hz gave {frames} frames");
        }
    }

    #[test]
    fn playback_stretches_a_frame_to_the_device_rate() {
        let mut converter = FromFrames::new(44_100);
        let mut fetched = 0;
        for _ in 0..44_100 {
            let value = converter.sample(|| {
                fetched += 1;
                Some([0.25; FRAME])
            });
            assert!(value.abs() <= 0.25 + 1e-6);
        }
        assert!((50..=51).contains(&fetched), "fetched {fetched}");
    }
}
