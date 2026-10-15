use gpui::{Motion, SpringConfig, ease_in_out, ease_out_quint, linear};
use std::time::Duration;

/// Standard durations, so related motion moves at related speeds.
#[derive(Clone, Copy, Debug)]
pub enum Speed {
    /// Small feedback: chips, icons, hover reveals. Defaults to 120 ms.
    Quick,
    /// Most entrances: bars, menus, panels. Defaults to 200 ms.
    Standard,
    /// Large or prominent surfaces. Defaults to 320 ms.
    Gentle,
}

/// How motion progresses over its duration.
#[derive(Clone, Copy, Debug)]
pub enum Curve {
    /// Constant speed.
    Linear,
    /// Starts fast and settles gently. The default for entrances.
    EaseOut,
    /// Starts and ends slowly.
    EaseInOut,
    /// Overshoots its target a little and settles back, for a playful pop.
    Overshoot,
    /// A physical spring sampled over the duration.
    Spring(SpringConfig),
}

impl Curve {
    /// A firm spring that settles quickly with almost no bounce.
    pub const SNAPPY: Self = Self::Spring(SpringConfig::new(420.0, 32.0, 1.0));
    /// A softer spring with a visible bounce.
    pub const BOUNCY: Self = Self::Spring(BOUNCY_SPRING);

    pub(crate) fn motion(self, duration: Duration) -> Motion {
        let motion = Motion::new(duration);
        match self {
            Self::Linear => motion.with_easing(linear),
            Self::EaseOut => motion.with_easing(ease_out_quint()),
            Self::EaseInOut => motion.with_easing(ease_in_out),
            Self::Overshoot => motion.with_easing(ease_out_back),
            Self::Spring(config) => motion.with_spring(config),
        }
    }
}

pub const BOUNCY_SPRING: SpringConfig = SpringConfig::new(300.0, 14.0, 1.0);

/// The value `progress` of the way from `from` to `to`.
pub fn mix(from: f32, to: f32, progress: f32) -> f32 {
    from + (to - from) * progress
}

/// Linearly interpolates between `from` and `to` at `progress`.
pub fn lerp(from: f32, to: f32, progress: f32) -> f32 {
    mix(from, to, progress)
}

/// Starts and ends gently, for motion that may reverse.
pub fn ease_in_out_cubic(delta: f32) -> f32 {
    if delta < 0.5 {
        4.0 * delta * delta * delta
    } else {
        1.0 - (-2.0 * delta + 2.0).powi(3) / 2.0
    }
}

/// Starts slowly and accelerates away.
pub(crate) fn ease_in_cubic(delta: f32) -> f32 {
    delta * delta * delta
}

/// Eases out past the target and settles back.
pub(crate) fn ease_out_back(delta: f32) -> f32 {
    const OVERSHOOT: f32 = 1.70158;
    let t = delta - 1.0;
    1.0 + (OVERSHOOT + 1.0) * t * t * t + OVERSHOOT * t * t
}
