use std::time::{Duration, Instant};

use crate::{
    MotionConfig, Speed,
    curve::{ease_in_out_cubic, mix},
};

/// Interpolation curve used by [`Tween`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TweenCurve {
    /// Constant speed.
    Linear,
    /// Smoothstep interpolation, retained for layout motion that used it before extraction.
    SmoothStep,
    /// Starts and ends gently; the default for interruptible values.
    #[default]
    EaseInOutCubic,
}

/// A value moving smoothly between scalar endpoints, such as a sidebar's
/// openness or an element's temporary layout offset.
#[derive(Clone, Copy, Debug)]
pub struct Tween {
    from: f32,
    to: f32,
    started: Instant,
    duration: Duration,
    curve: TweenCurve,
}

impl Tween {
    /// A tween resting at `value`.
    pub fn at(value: f32) -> Self {
        Self::at_with_curve(value, TweenCurve::default())
    }

    /// A tween resting at `value` with an explicit default interpolation curve.
    pub fn at_with_curve(value: f32, curve: TweenCurve) -> Self {
        Self {
            from: value,
            to: value,
            started: Instant::now(),
            duration: Duration::ZERO,
            curve,
        }
    }

    /// Starts or redirects the tween using the configured named duration.
    /// If motion is disallowed, the value snaps even when the target is unchanged.
    pub fn animate_to(&mut self, target: f32, speed: Speed, config: &MotionConfig) {
        self.animate_to_for(target, config.duration(speed), config);
    }

    /// Starts or redirects the tween using an explicit duration.
    /// Disallowed motion snaps to the final value immediately.
    pub fn animate_to_for(&mut self, target: f32, duration: Duration, config: &MotionConfig) {
        let now = Instant::now();
        if !config.allows_motion() {
            self.jump_to_at(target, now);
            return;
        }
        self.animate_to_at(target, duration, self.curve, now);
    }

    /// Starts or redirects the tween at an explicit timestamp and curve.
    /// Reversing an active tween samples its current value first, preserving continuity.
    pub fn animate_to_at(
        &mut self,
        target: f32,
        duration: Duration,
        curve: TweenCurve,
        now: Instant,
    ) {
        if self.to == target && !duration.is_zero() {
            return;
        }
        self.from = self.value_at(now);
        self.to = target;
        self.started = now;
        self.duration = duration;
        self.curve = curve;
    }

    /// Rests at `value` at once.
    pub fn jump_to(&mut self, value: f32) {
        self.jump_to_at(value, Instant::now());
    }

    /// Rests at `value` at an explicit timestamp.
    pub fn jump_to_at(&mut self, value: f32, now: Instant) {
        self.from = value;
        self.to = value;
        self.started = now;
        self.duration = Duration::ZERO;
    }

    /// Returns the sampled value at an explicit timestamp.
    pub fn value_at(&self, now: Instant) -> f32 {
        if self.duration.is_zero() {
            return self.to;
        }
        let t = (now.saturating_duration_since(self.started).as_secs_f32()
            / self.duration.as_secs_f32())
        .clamp(0.0, 1.0);
        mix(self.from, self.to, self.curve.apply(t))
    }

    /// Returns the value, settling immediately if the current policy disallows motion.
    pub fn value_with_config(&self, now: Instant, config: &MotionConfig) -> f32 {
        if config.allows_motion() {
            self.value_at(now)
        } else {
            self.to
        }
    }

    /// Returns the current value using the system clock.
    pub fn value(&self) -> f32 {
        self.value_at(Instant::now())
    }

    /// The destination value.
    pub fn target(&self) -> f32 {
        self.to
    }

    /// Whether the tween is active at an explicit timestamp.
    pub fn is_running_at(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.started) < self.duration
    }

    /// Whether this tween should continue requesting frames under `config`.
    pub fn is_running_with_config(&self, now: Instant, config: &MotionConfig) -> bool {
        config.allows_motion() && self.is_running_at(now)
    }

    /// Whether the tween is active using the system clock.
    pub fn is_running(&self) -> bool {
        self.is_running_at(Instant::now())
    }
}

impl TweenCurve {
    fn apply(self, t: f32) -> f32 {
        match self {
            Self::Linear => t,
            Self::SmoothStep => t * t * (3.0 - 2.0 * t),
            Self::EaseInOutCubic => ease_in_out_cubic(t),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Tween, TweenCurve};
    use crate::{MotionConfig, MotionTimings};
    use std::time::{Duration, Instant};

    #[test]
    fn reversing_a_tween_preserves_continuity() {
        let start = Instant::now();
        let mut tween = Tween::at(0.0);
        tween.animate_to_at(
            1.0,
            Duration::from_millis(200),
            TweenCurve::EaseInOutCubic,
            start,
        );

        let halfway = start + Duration::from_millis(100);
        let before = tween.value_at(halfway);
        tween.animate_to_at(
            0.0,
            Duration::from_millis(200),
            TweenCurve::EaseInOutCubic,
            halfway,
        );
        assert!((tween.value_at(halfway) - before).abs() < 0.00001);
    }

    #[test]
    fn completion_and_zero_duration_are_exact() {
        let start = Instant::now();
        let mut tween = Tween::at(2.0);
        tween.animate_to_at(8.0, Duration::from_millis(100), TweenCurve::Linear, start);
        assert_eq!(tween.value_at(start + Duration::from_millis(100)), 8.0);
        assert!(!tween.is_running_at(start + Duration::from_millis(100)));

        tween.animate_to_at(4.0, Duration::ZERO, TweenCurve::Linear, start);
        assert_eq!(tween.value_at(start), 4.0);
    }

    #[test]
    fn repeated_target_does_not_restart_an_active_tween() {
        let start = Instant::now();
        let mut tween = Tween::at(0.0);
        tween.animate_to_at(1.0, Duration::from_millis(200), TweenCurve::Linear, start);
        let midpoint = start + Duration::from_millis(100);
        tween.animate_to_at(
            1.0,
            Duration::from_millis(300),
            TweenCurve::SmoothStep,
            midpoint,
        );
        assert_eq!(tween.value_at(midpoint), 0.5);
        assert_eq!(tween.value_at(start + Duration::from_millis(200)), 1.0);
    }

    #[test]
    fn disallowed_motion_snaps_even_when_target_is_unchanged() {
        let start = Instant::now();
        let mut tween = Tween::at(0.0);
        tween.animate_to_at(1.0, Duration::from_secs(1), TweenCurve::Linear, start);
        let disabled = MotionConfig {
            enabled: false,
            timings: MotionTimings::default(),
            ..MotionConfig::default()
        };
        tween.animate_to_for(1.0, Duration::from_secs(1), &disabled);
        assert_eq!(tween.value_with_config(start, &disabled), 1.0);
        assert!(!tween.is_running_with_config(start, &disabled));
    }
}
