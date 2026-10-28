//! Noise suppression with RNNoise (`nnnoiseless`), which works on 10 ms of
//! 48 kHz sound, two to a frame.

use nnnoiseless::DenoiseState;

use crate::{FRAME, Frame};

/// RNNoise wants samples at 16-bit scale.
const SCALE: f32 = 32_767.;

pub struct Denoiser {
    state: Box<DenoiseState<'static>>,
    /// RNNoise's first output is garbage while its state settles.
    warm: bool,
}

impl Default for Denoiser {
    fn default() -> Self {
        Self { state: DenoiseState::new(), warm: false }
    }
}

impl Denoiser {
    /// Cleans `frame` in place.
    pub fn process(&mut self, frame: &mut Frame) {
        let mut input = [0.; DenoiseState::FRAME_SIZE];
        let mut output = [0.; DenoiseState::FRAME_SIZE];
        for half in frame.as_chunks_mut::<{ DenoiseState::FRAME_SIZE }>().0 {
            for (scaled, sample) in input.iter_mut().zip(half.iter()) {
                *scaled = sample.clamp(-1., 1.) * SCALE;
            }
            self.state.process_frame(&mut output, &input);
            if self.warm {
                for (sample, cleaned) in half.iter_mut().zip(output.iter()) {
                    *sample = cleaned / SCALE;
                }
            }
            self.warm = true;
        }
    }
}

const _: () = assert!(FRAME == 2 * DenoiseState::FRAME_SIZE);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_stays_silent() {
        let mut denoiser = Denoiser::default();
        for _ in 0..5 {
            let mut frame = [0.; FRAME];
            denoiser.process(&mut frame);
            assert!(frame.iter().all(|s| s.abs() < 1e-3));
        }
    }
}
