//! Small procedural sounds for Photon interaction feedback.
//!
//! The generator is deliberately independent of audio devices. Callers can
//! render short mono samples and send them to any playback backend.

/// Semantic interactions with a distinct sound character.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Sound {
    /// A quiet, rounded control click.
    StateChange,
    /// A slightly brighter alignment tick.
    Alignment,
}

/// Central sound design controls. Values are easy to tune without changing
/// the renderer or its callers.
#[derive(Clone, Copy, Debug)]
pub struct SoundConfig {
    /// Sample rate used by the renderer and output backend.
    pub sample_rate: u32,
    /// Linear output gain, from 0.0 (silent) to 1.0 (full scale).
    pub gain: f32,
    /// Maximum variation in the filter cutoff, in hertz.
    pub cutoff_variation_hz: f32,
}

impl Default for SoundConfig {
    fn default() -> Self {
        Self {
            sample_rate: 24_000,
            // UI feedback should sit well below normal page and system audio.
            gain: 0.035,
            cutoff_variation_hz: 140.0,
        }
    }
}

/// A rendered mono sound.
#[derive(Clone, Debug)]
pub struct SoundBuffer {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

/// Render a short, soft synthesized interaction sound.
pub fn render(sound: Sound, variation: f32, config: SoundConfig) -> SoundBuffer {
    let sample_rate = config.sample_rate.max(8_000);
    let (duration, decay_samples, base_cutoff, q, level) = match sound {
        Sound::StateChange => (0.004, 25.0, 3_000.0, 8.0, 1.0),
        Sound::Alignment => (0.003, 19.0, 3_600.0, 8.0, 0.78),
    };
    let cutoff = (base_cutoff + variation.clamp(-1.0, 1.0) * config.cutoff_variation_hz)
        .clamp(200.0, sample_rate as f32 * 0.45);
    let count = (duration * sample_rate as f32) as usize;
    let mut state = (variation.to_bits() ^ 0xa341_316c).max(1);
    let mut samples = (0..count)
        .map(|index| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            let noise = (state as i32 as f32) / i32::MAX as f32;
            let decay = (-(index as f32) / decay_samples).exp();
            noise * decay * level
        })
        .collect::<Vec<_>>();
    bandpass(&mut samples, cutoff, q, sample_rate as f32);

    // Normalize each stochastic burst to a stable peak before applying gain.
    let peak = samples
        .iter()
        .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
    let gain = config.gain.clamp(0.0, 1.0);
    if peak > f32::EPSILON {
        for sample in &mut samples {
            *sample = (*sample / peak * gain).clamp(-1.0, 1.0);
        }
    }
    SoundBuffer {
        samples,
        sample_rate,
    }
}

fn bandpass(samples: &mut [f32], frequency: f32, q: f32, sample_rate: f32) {
    let w0 = std::f32::consts::TAU * frequency / sample_rate;
    let alpha = w0.sin() / (2.0 * q);
    let cos_w = w0.cos();
    let b0 = alpha;
    let b2 = -alpha;
    let a0 = 1.0 + alpha;
    let a1 = -2.0 * cos_w;
    let a2 = 1.0 - alpha;
    let (mut x1, mut x2, mut y1, mut y2) = (0.0, 0.0, 0.0, 0.0);

    for sample in samples {
        let input = *sample;
        let output = (b0 * input + b2 * x2 - a1 * y1 - a2 * y2) / a0;
        (x2, x1) = (x1, input);
        (y2, y1) = (y1, output);
        *sample = output;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_short_bounded_audio() {
        for sound in [Sound::StateChange, Sound::Alignment] {
            let buffer = render(sound, 0.0, SoundConfig::default());
            assert!(!buffer.samples.is_empty());
            assert!(buffer.samples.len() < 1_000);
            assert!(
                buffer
                    .samples
                    .iter()
                    .all(|sample| sample.is_finite() && sample.abs() <= 1.0)
            );
        }
    }

    #[test]
    fn rendered_peak_respects_the_configured_gain() {
        let config = SoundConfig::default();
        for sound in [Sound::StateChange, Sound::Alignment] {
            let buffer = render(sound, 0.0, config);
            let peak = buffer
                .samples
                .iter()
                .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
            assert!((peak - config.gain).abs() < 0.0001);
        }
    }
}
