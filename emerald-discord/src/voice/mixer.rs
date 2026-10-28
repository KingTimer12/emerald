//! Everyone else in the call, mixed into one frame each tick: a decoder and
//! a [`Jitter`] buffer per speaker, found by the SSRC their packets carry.

use emerald_audio::{FRAME, Frame};
use opus2::{Channels, Decoder};

use super::jitter::{Jitter, Next};

/// Discord's cap on people in one voice channel.
pub const MAX_SPEAKERS: usize = 99;
/// Samples in the longest Opus packet, 120 ms.
const MAX_DECODED: usize = 5760;
/// A packet within this many ticks counts as speaking: 200 ms.
const SPEAKING_TICKS: u8 = 10;
/// Quieter than this is not speech, whatever packets arrive.
const SPEECH_LEVEL: f32 = -55.;

struct Speaker {
    user: u64,
    ssrc: u32,
    decoder: Decoder,
    jitter: Jitter,
    /// Decoded samples not yet mixed, from `read` to `filled`.
    decoded: Box<[f32; MAX_DECODED]>,
    read: usize,
    filled: usize,
    /// Ticks left of the speaking ring.
    speaking: u8,
    /// Percent; 100 unless the user turned this person up or down.
    volume: u16,
}

#[derive(Default)]
pub struct Mixer {
    speakers: Vec<Speaker>,
}

impl Mixer {
    /// `user` sends on `ssrc`. A user takes one SSRC at a time.
    pub fn announce(&mut self, user: u64, ssrc: u32) {
        if self.speakers.iter().any(|s| s.user == user && s.ssrc == ssrc) {
            return;
        }
        self.speakers.retain(|s| s.user != user && s.ssrc != ssrc);
        if self.speakers.len() >= MAX_SPEAKERS {
            return;
        }
        let Ok(decoder) = Decoder::new(emerald_audio::RATE, Channels::Mono) else {
            tracing::warn!("voice: could not create an Opus decoder");
            return;
        };
        self.speakers.push(Speaker {
            user,
            ssrc,
            decoder,
            jitter: Jitter::default(),
            decoded: Box::new([0.; MAX_DECODED]),
            read: 0,
            filled: 0,
            speaking: 0,
            volume: 100,
        });
    }

    pub fn remove(&mut self, user: u64) {
        self.speakers.retain(|s| s.user != user);
    }

    /// The SSRC `user` sends on.
    pub fn ssrc(&self, user: u64) -> Option<u32> {
        self.speakers.iter().find(|s| s.user == user).map(|s| s.ssrc)
    }

    pub fn user(&self, ssrc: u32) -> Option<u64> {
        self.speakers.iter().find(|s| s.ssrc == ssrc).map(|s| s.user)
    }

    pub fn push(&mut self, ssrc: u32, sequence: u16, opus: Vec<u8>) {
        if let Some(speaker) = self.speakers.iter_mut().find(|s| s.ssrc == ssrc) {
            speaker.jitter.push(sequence, opus);
        }
    }

    /// Forgets what was buffered, as after deafening.
    pub fn clear(&mut self) {
        for speaker in &mut self.speakers {
            speaker.jitter.clear();
            speaker.read = 0;
            speaker.filled = 0;
            speaker.speaking = 0;
        }
    }

    /// Those heard in the last 200 ms.
    pub fn speaking(&self) -> impl Iterator<Item = u64> + '_ {
        self.speakers.iter().filter(|s| s.speaking > 0).map(|s| s.user)
    }

    /// One frame of everyone together; `None` when nobody made a sound.
    pub fn mix(&mut self) -> Option<Frame> {
        let mut mixed = [0.; FRAME];
        let mut heard = false;
        for speaker in &mut self.speakers {
            speaker.speaking = speaker.speaking.saturating_sub(1);
            let mut frame = [0.; FRAME];
            if !speaker.fill(&mut frame) {
                continue;
            }
            if emerald_audio::level(&frame) > SPEECH_LEVEL {
                speaker.speaking = SPEAKING_TICKS;
            }
            let gain = f32::from(speaker.volume) / 100.;
            for (out, sample) in mixed.iter_mut().zip(frame) {
                *out += sample * gain;
            }
            heard = true;
        }
        heard.then(|| mixed.map(|s| s.clamp(-1., 1.)))
    }
}

impl Speaker {
    /// Fills `frame` from what was decoded, decoding more as needed; false
    /// when there was nothing to play.
    fn fill(&mut self, frame: &mut Frame) -> bool {
        let mut written = 0;
        // The shortest packet is 2.5 ms, so a frame never needs more.
        for _ in 0..8 {
            if written == FRAME {
                break;
            }
            if self.read == self.filled && !self.decode() {
                break;
            }
            let take = (self.filled - self.read).min(FRAME - written);
            frame[written..written + take].copy_from_slice(&self.decoded[self.read..self.read + take]);
            self.read += take;
            written += take;
        }
        written > 0
    }

    fn decode(&mut self) -> bool {
        let decoded = match self.jitter.pop() {
            Next::Packet(opus) => self.decoder.decode_float(&opus, &mut self.decoded[..], false),
            // Concealment as long as the last packet, so timing holds.
            Next::Lost => {
                let length = self.decoder.get_last_packet_duration().unwrap_or(FRAME as u32) as usize;
                let length = length.clamp(120, MAX_DECODED);
                self.decoder.decode_float(&[], &mut self.decoded[..length], false)
            }
            Next::Silence => return false,
        };
        match decoded {
            Ok(samples) if samples > 0 => {
                self.read = 0;
                self.filled = samples;
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opus2::{Application, Encoder};

    fn tone(amplitude: f32) -> Vec<u8> {
        let mut encoder = Encoder::new(emerald_audio::RATE, Channels::Mono, Application::Voip).unwrap();
        let samples: Vec<f32> = (0..FRAME).map(|i| amplitude * (i as f32 * 0.05).sin()).collect();
        let mut out = vec![0; 1275];
        let length = encoder.encode_float(&samples, &mut out).unwrap();
        out.truncate(length);
        out
    }

    #[test]
    fn speakers_are_found_by_ssrc_and_heard_after_the_delay() {
        let mut mixer = Mixer::default();
        mixer.announce(5, 100);
        assert_eq!(mixer.user(100), Some(5));
        for sequence in 0..4 {
            mixer.push(100, sequence, tone(0.5));
        }
        // Unknown SSRCs are dropped.
        mixer.push(200, 0, tone(0.5));
        assert!(mixer.mix().is_none());
        assert!(mixer.mix().is_none());
        assert!(mixer.mix().is_some());
        assert_eq!(mixer.speaking().collect::<Vec<_>>(), vec![5]);
        mixer.remove(5);
        assert_eq!(mixer.user(100), None);
    }

    #[test]
    fn a_new_ssrc_replaces_the_old_one() {
        let mut mixer = Mixer::default();
        mixer.announce(5, 100);
        mixer.announce(5, 101);
        assert_eq!((mixer.user(100), mixer.user(101)), (None, Some(5)));
    }
}
