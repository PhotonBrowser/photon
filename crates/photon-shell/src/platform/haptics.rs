//! Semantic interaction feedback shared by browser UI components.

use gpui::{App, HapticFeedbackStyle};
use photon_sound::{Sound, SoundConfig, render};
use rodio::{OutputStream, OutputStreamHandle, Sink, buffer::SamplesBuffer};
use std::sync::{OnceLock, mpsc};

/// The interaction represented by a feedback event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feedback {
    /// A discrete control state change, such as a switch changing sides.
    StateChange,
    /// Crossing an alignment point, detent, or snap threshold.
    Alignment,
}

/// Which feedback channels to use for a semantic interaction.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Channels {
    pub haptic: bool,
    pub sound: bool,
}

impl Channels {
    pub const HAPTIC: Self = Self {
        haptic: true,
        sound: false,
    };
    pub const SOUND: Self = Self {
        haptic: false,
        sound: true,
    };
    pub const BOTH: Self = Self {
        haptic: true,
        sound: true,
    };
}

/// Plays semantic feedback using whichever requested channels are available.
/// Audio failure is intentionally silent and never prevents the UI action.
pub fn play(cx: &App, feedback: Feedback) {
    play_with(cx, feedback, Channels::BOTH);
}

/// Plays only the audible part of a semantic interaction.
#[allow(dead_code)]
pub fn play_sound_only(cx: &App, feedback: Feedback) {
    play_with(cx, feedback, Channels::SOUND);
}

/// Plays only the device haptic for a semantic interaction.
#[allow(dead_code)]
pub fn play_haptic_only(cx: &App, feedback: Feedback) {
    play_with(cx, feedback, Channels::HAPTIC);
}

/// Plays feedback through explicitly selected channels. Use `SOUND` for
/// interactions that benefit from audible feedback without a device haptic.
#[allow(dead_code)]
pub fn play_with(cx: &App, feedback: Feedback, channels: Channels) {
    if channels.haptic {
        let style = match feedback {
            Feedback::StateChange => HapticFeedbackStyle::LevelChange,
            Feedback::Alignment => HapticFeedbackStyle::Alignment,
        };
        cx.play_haptic_feedback(style);
    }

    if channels.sound && crate::platform::ui::interaction_sounds_enabled(cx) {
        play_sound(feedback);
    }
}

fn play_sound(feedback: Feedback) {
    static PLAYER: OnceLock<mpsc::Sender<(Sound, f32)>> = OnceLock::new();
    let sender = PLAYER.get_or_init(|| {
        let (sender, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("photon-ui-audio".into())
            .spawn(move || {
                let mut player = None;
                while let Ok((sound, pitch)) = receiver.recv() {
                    if player.is_none() {
                        player = AudioPlayer::new().ok();
                    }
                    if let Some(player) = player.as_mut() {
                        let buffer = render(sound, pitch, SoundConfig::default());
                        player.play(buffer.samples, buffer.sample_rate);
                    }
                }
            })
            .ok();
        sender
    });
    let (sound, seed) = match feedback {
        Feedback::StateChange => (Sound::StateChange, next_pitch_seed()),
        Feedback::Alignment => (Sound::Alignment, next_pitch_seed()),
    };
    let variation = (seed - 0.5) * 2.0;
    let _ = sender.send((sound, variation));
}

fn next_pitch_seed() -> f32 {
    static SEQUENCE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let value = SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    // Deterministic low-discrepancy variation avoids randomness at each click.
    ((value.wrapping_mul(2_654_435_761) >> 8) & 0x00ff_ffff) as f32 / 16_777_216.0
}

struct AudioPlayer {
    // Keeping the stream alive keeps the operating-system output device open.
    _stream: OutputStream,
    handle: OutputStreamHandle,
    sinks: Vec<Sink>,
}

impl AudioPlayer {
    fn new() -> Result<Self, rodio::StreamError> {
        let (stream, handle) = OutputStream::try_default()?;
        Ok(Self {
            _stream: stream,
            handle,
            sinks: Vec::new(),
        })
    }

    fn play(&mut self, samples: Vec<f32>, sample_rate: u32) {
        self.sinks.retain(|sink| !sink.empty());
        let Ok(sink) = Sink::try_new(&self.handle) else {
            return;
        };
        sink.append(SamplesBuffer::new(1, sample_rate, samples));
        self.sinks.push(sink);
    }
}
