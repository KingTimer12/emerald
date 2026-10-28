//! The open microphone and speaker streams, and the switches the caller
//! flips while they run. Device callbacks run on the system's audio
//! thread: they only convert samples and move frames through lock-free
//! rings, never wait or allocate.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};

use anyhow::{Result, anyhow};
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, StreamConfig, SupportedStreamConfig};

use crate::devices::pick;
use crate::resample::{FromFrames, ToFrames};
use crate::{Frame, RATE};

/// Frames each ring holds: 160 ms, enough for a late tick or two.
const RING: usize = 8;

/// Captured frames, oldest first.
pub type Microphone = rtrb::Consumer<Frame>;
/// Frames to play, oldest first; a full ring drops the newest.
pub type Speaker = rtrb::Producer<Frame>;

/// Which devices to open: by [`crate::Device::id`], or the system default
/// when `None` or when the chosen one is gone.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Choice {
    pub input: Option<String>,
    pub output: Option<String>,
}

/// What the caller changes while the streams run.
struct Switches {
    muted: AtomicBool,
    deafened: AtomicBool,
    /// Percent, 0 to 200.
    input_volume: AtomicU16,
    output_volume: AtomicU16,
    /// A device failed or went away; reopen to recover.
    failed: AtomicBool,
}

/// Open streams; dropping it closes them.
pub struct Audio {
    switches: Arc<Switches>,
    _input: Option<cpal::Stream>,
    _output: Option<cpal::Stream>,
    microphone_failed: bool,
}

impl Audio {
    /// Opens the chosen devices. Fails only when neither can be opened.
    pub fn open(choice: &Choice) -> Result<(Self, Microphone, Speaker)> {
        let host = cpal::default_host();
        let switches = Arc::new(Switches {
            muted: AtomicBool::new(false),
            deafened: AtomicBool::new(false),
            input_volume: AtomicU16::new(100),
            output_volume: AtomicU16::new(100),
            failed: AtomicBool::new(false),
        });
        let (capture_in, microphone) = rtrb::RingBuffer::new(RING);
        let (speaker, playback_out) = rtrb::RingBuffer::new(RING);

        let input = pick(&host, choice.input.as_deref(), true)
            .ok_or_else(|| anyhow!("no microphone"))
            .and_then(|device| open_input(&device, capture_in, switches.clone()))
            .inspect_err(|e| tracing::warn!("audio: microphone unavailable: {e:#}"))
            .ok();
        let output = pick(&host, choice.output.as_deref(), false)
            .ok_or_else(|| anyhow!("no speaker"))
            .and_then(|device| open_output(&device, playback_out, switches.clone()))
            .inspect_err(|e| tracing::warn!("audio: speaker unavailable: {e:#}"))
            .ok();
        if input.is_none() && output.is_none() {
            return Err(anyhow!("no audio device could be opened"));
        }
        let audio = Self { switches, microphone_failed: input.is_none(), _input: input, _output: output };
        Ok((audio, microphone, speaker))
    }

    /// No microphone could be opened; the call is listen-only.
    pub fn microphone_failed(&self) -> bool {
        self.microphone_failed
    }

    /// A device failed or was unplugged since it opened.
    pub fn failed(&self) -> bool {
        self.switches.failed.load(Ordering::Relaxed)
    }

    /// Stops capturing; frames already captured are kept.
    pub fn set_muted(&self, muted: bool) {
        self.switches.muted.store(muted, Ordering::Relaxed);
    }

    /// Plays silence and drops what was waiting to play.
    pub fn set_deafened(&self, deafened: bool) {
        self.switches.deafened.store(deafened, Ordering::Relaxed);
    }

    /// Gains in percent, up to 200.
    pub fn set_volumes(&self, input: u16, output: u16) {
        self.switches.input_volume.store(input.min(200), Ordering::Relaxed);
        self.switches.output_volume.store(output.min(200), Ordering::Relaxed);
    }
}

fn gain(volume: &AtomicU16) -> f32 {
    f32::from(volume.load(Ordering::Relaxed)) / 100.
}

/// 48 kHz when the device offers it, so nothing needs converting; else its
/// own default.
fn input_config(device: &cpal::Device) -> Result<SupportedStreamConfig> {
    let native = device.supported_input_configs().ok().and_then(|configs| {
        configs
            .filter(|c| (1..=8).contains(&c.channels()) && supported(c.sample_format()))
            .filter_map(|c| c.try_with_sample_rate(RATE))
            .min_by_key(|c| c.channels())
    });
    match native {
        Some(config) => Ok(config),
        None => Ok(device.default_input_config()?),
    }
}

fn supported(format: SampleFormat) -> bool {
    matches!(format, SampleFormat::F32 | SampleFormat::I16 | SampleFormat::I32 | SampleFormat::U16)
}

/// Errors that leave the stream running.
fn harmless(error: &cpal::Error) -> bool {
    matches!(error.kind(), cpal::ErrorKind::Xrun | cpal::ErrorKind::RealtimeDenied | cpal::ErrorKind::DeviceChanged)
}

fn open_input(device: &cpal::Device, ring: rtrb::Producer<Frame>, switches: Arc<Switches>) -> Result<cpal::Stream> {
    let supported = input_config(device)?;
    let config = supported.config();
    let stream = match supported.sample_format() {
        SampleFormat::F32 => input_stream::<f32>(device, &config, ring, switches),
        SampleFormat::I16 => input_stream::<i16>(device, &config, ring, switches),
        SampleFormat::I32 => input_stream::<i32>(device, &config, ring, switches),
        SampleFormat::U16 => input_stream::<u16>(device, &config, ring, switches),
        format => return Err(anyhow!("microphone sample format {format} is not supported")),
    }?;
    stream.play()?;
    tracing::info!(rate = config.sample_rate, channels = config.channels, "audio: microphone open");
    Ok(stream)
}

fn input_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    mut ring: rtrb::Producer<Frame>,
    switches: Arc<Switches>,
) -> Result<cpal::Stream>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = usize::from(config.channels);
    let mut converter = ToFrames::new(config.sample_rate);
    let failed = switches.clone();
    let stream = device.build_input_stream(
        *config,
        move |data: &[T], _| {
            if switches.muted.load(Ordering::Relaxed) {
                converter.reset();
                return;
            }
            let gain = gain(&switches.input_volume);
            for samples in data.chunks_exact(channels) {
                // Mixed down to mono.
                let sample = samples.iter().map(|s| s.to_sample::<f32>()).sum::<f32>() / channels as f32;
                let sample = if sample.is_finite() { (sample * gain).clamp(-1., 1.) } else { 0. };
                // A full ring means the reader is late; the newest frame waits.
                converter.push(sample, |frame| {
                    ring.push(frame).ok();
                });
            }
        },
        move |error| {
            if !harmless(&error) {
                failed.failed.store(true, Ordering::Relaxed);
            }
        },
        None,
    )?;
    Ok(stream)
}

fn open_output(device: &cpal::Device, ring: rtrb::Consumer<Frame>, switches: Arc<Switches>) -> Result<cpal::Stream> {
    let supported = device.default_output_config()?;
    let config = supported.config();
    let stream = match supported.sample_format() {
        SampleFormat::F32 => output_stream::<f32>(device, &config, ring, switches),
        SampleFormat::I16 => output_stream::<i16>(device, &config, ring, switches),
        SampleFormat::I32 => output_stream::<i32>(device, &config, ring, switches),
        SampleFormat::U16 => output_stream::<u16>(device, &config, ring, switches),
        format => return Err(anyhow!("speaker sample format {format} is not supported")),
    }?;
    stream.play()?;
    tracing::info!(rate = config.sample_rate, channels = config.channels, "audio: speaker open");
    Ok(stream)
}

fn output_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    mut ring: rtrb::Consumer<Frame>,
    switches: Arc<Switches>,
) -> Result<cpal::Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = usize::from(config.channels);
    let mut converter = FromFrames::new(config.sample_rate);
    let failed = switches.clone();
    let stream = device.build_output_stream(
        *config,
        move |data: &mut [T], _| {
            if switches.deafened.load(Ordering::Relaxed) {
                while ring.pop().is_ok() {}
                data.fill(T::from_sample(0.));
                return;
            }
            let gain = gain(&switches.output_volume);
            for samples in data.chunks_mut(channels) {
                let sample = converter.sample(|| ring.pop().ok());
                let sample = if sample.is_finite() { (sample * gain).clamp(-1., 1.) } else { 0. };
                samples.fill(T::from_sample(sample));
            }
        },
        move |error| {
            if !harmless(&error) {
                failed.failed.store(true, Ordering::Relaxed);
            }
        },
        None,
    )?;
    Ok(stream)
}
