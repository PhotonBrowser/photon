use std::time::Duration;

use crate::Speed;

/// Application supplied timing and accessibility preferences for motion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionConfig {
    /// Whether the application has enabled decorative animations.
    pub enabled: bool,
    /// Whether the current platform requests reduced motion.
    pub reduce_motion: bool,
    /// Relative size of movement and blur effects, where `1.0` is the default.
    pub effect_strength: f32,
    /// Named timing tiers used by effects and tweens.
    pub timings: MotionTimings,
}

impl MotionConfig {
    /// Whether an animation may run under both application and platform policy.
    pub const fn allows_motion(self) -> bool {
        self.enabled && !self.reduce_motion
    }

    /// Returns the configured duration, or zero when motion is disabled.
    pub const fn duration(self, speed: Speed) -> Duration {
        if !self.allows_motion() {
            return Duration::ZERO;
        }
        match speed {
            Speed::Quick => self.timings.quick,
            Speed::Standard => self.timings.standard,
            Speed::Gentle => self.timings.gentle,
        }
    }
}

impl Default for MotionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            reduce_motion: false,
            effect_strength: 1.0,
            timings: MotionTimings::default(),
        }
    }
}

/// Durations for common classes of motion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MotionTimings {
    /// Small feedback, icons and hover reveals.
    pub quick: Duration,
    /// Common panels, menus and controls.
    pub standard: Duration,
    /// Larger, prominent surfaces.
    pub gentle: Duration,
}

impl Default for MotionTimings {
    fn default() -> Self {
        Self {
            quick: Duration::from_millis(120),
            standard: Duration::from_millis(200),
            gentle: Duration::from_millis(320),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MotionConfig, MotionTimings};
    use crate::Speed;
    use std::time::Duration;

    #[test]
    fn named_durations_follow_accessibility_policy() {
        let config = MotionConfig::default();
        assert_eq!(config.duration(Speed::Quick), Duration::from_millis(120));

        let disabled = MotionConfig {
            enabled: false,
            ..config
        };
        assert_eq!(disabled.duration(Speed::Quick), Duration::ZERO);

        let reduced = MotionConfig {
            reduce_motion: true,
            ..config
        };
        assert_eq!(reduced.duration(Speed::Gentle), Duration::ZERO);
        assert_eq!(config.timings, MotionTimings::default());
    }
}
