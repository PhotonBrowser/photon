use crate::curve::ease_in_cubic;
use crate::{Curve, MotionConfig, Speed};
use gpui::{Animation, Motion, Styled, px};
use std::time::Duration;

/// The edge an element slides in from.
#[derive(Clone, Copy, Debug)]
pub enum Edge {
    /// Enter from above.
    Top,
    /// Enter from below.
    Bottom,
    /// Enter from the left.
    Left,
    /// Enter from the right.
    Right,
}

/// How an element appears. Compose effects with the builder API, then apply it
/// with [`crate::AnimateIn::animate_in`].
#[derive(Clone, Copy, Debug)]
pub struct Entrance {
    speed: Speed,
    duration_override: Option<Duration>,
    delay: Duration,
    curve: Curve,
    /// The opacity the element starts at.
    opacity_from: Option<f32>,
    /// The offset the element starts at, in pixels.
    offset_from: Option<(f32, f32)>,
    /// The blur radius the element starts at, in pixels.
    blur_from: Option<f32>,
    /// The element's final size and the fraction of it the element starts at.
    grow: Option<(f32, f32)>,
    /// The uniform scale the element starts at.
    scale_from: Option<f32>,
}

impl Entrance {
    /// An entrance with no effects yet, at standard speed, easing out.
    pub const fn new() -> Self {
        Self {
            speed: Speed::Standard,
            duration_override: None,
            delay: Duration::ZERO,
            curve: Curve::EaseOut,
            opacity_from: None,
            offset_from: None,
            blur_from: None,
            grow: None,
            scale_from: None,
        }
    }

    /// Scales into place from a smaller size. For controls and compact surfaces.
    pub const fn scale_from(mut self, scale: f32) -> Self {
        self.scale_from = Some(scale);
        self
    }

    // Effects.

    /// Starts at `opacity` and fades to fully opaque.
    pub const fn fade_from(mut self, opacity: f32) -> Self {
        self.opacity_from = Some(opacity);
        self
    }

    /// Starts `distance` pixels toward `edge` and slides into place.
    pub const fn slide_from(mut self, edge: Edge, distance: f32) -> Self {
        self.offset_from = Some(match edge {
            Edge::Top => (0.0, -distance),
            Edge::Bottom => (0.0, distance),
            Edge::Left => (-distance, 0.0),
            Edge::Right => (distance, 0.0),
        });
        self
    }

    /// Slides up into place from `distance` pixels below.
    pub const fn slide_up(self, distance: f32) -> Self {
        self.slide_from(Edge::Bottom, distance)
    }

    /// Slides down into place from `distance` pixels above.
    pub const fn slide_down(self, distance: f32) -> Self {
        self.slide_from(Edge::Top, distance)
    }

    /// Slides left into place from `distance` pixels to the right.
    pub const fn slide_left(self, distance: f32) -> Self {
        self.slide_from(Edge::Right, distance)
    }

    /// Slides right into place from `distance` pixels to the left.
    pub const fn slide_right(self, distance: f32) -> Self {
        self.slide_from(Edge::Left, distance)
    }

    /// Starts blurred by `radius` pixels and sharpens.
    pub const fn blur_from(mut self, radius: f32) -> Self {
        self.blur_from = Some(radius);
        self
    }

    /// Grows a square element to `size`, starting at `fraction` of it.
    pub const fn grow_from(mut self, size: f32, fraction: f32) -> Self {
        self.grow = Some((size, fraction));
        self
    }

    // Timing.

    /// Uses a named timing tier from [`MotionConfig::timings`].
    pub const fn speed(mut self, speed: Speed) -> Self {
        self.speed = speed;
        self.duration_override = None;
        self
    }

    /// Uses an explicit duration instead of a named timing tier.
    pub const fn duration(mut self, duration: Duration) -> Self {
        self.duration_override = Some(duration);
        self
    }

    /// Waits before starting, to stagger related entrances.
    pub fn delay(mut self, delay: impl Into<Duration>) -> Self {
        self.delay = delay.into();
        self
    }

    /// Uses the selected progress curve for the entrance.
    pub const fn curve(mut self, curve: Curve) -> Self {
        self.curve = curve;
        self
    }

    /// How long the entrance takes, including its delay.
    fn resolved_duration(&self, config: &MotionConfig) -> Duration {
        if !config.allows_motion() {
            return Duration::ZERO;
        }
        self.duration_override
            .unwrap_or_else(|| config.duration(self.speed))
    }

    /// How long the exit takes: quicker than the entrance, with no delay.
    pub fn exit_duration(&self, config: &MotionConfig) -> Duration {
        self.resolved_duration(config) * 3 / 4
    }

    /// How long the entrance takes, including its delay.
    pub fn total_duration(&self, config: &MotionConfig) -> Duration {
        if config.allows_motion() {
            self.delay + self.resolved_duration(config)
        } else {
            Duration::ZERO
        }
    }

    pub(crate) fn animation(&self, config: &MotionConfig) -> Animation {
        let duration = self.resolved_duration(config);
        let delay = if config.allows_motion() {
            self.delay
        } else {
            Duration::ZERO
        };
        Animation::new(self.curve.motion(duration).with_delay(delay))
    }

    /// The exit accelerates away rather than settling, whatever the entrance's curve.
    pub(crate) fn exit_animation(&self, config: &MotionConfig) -> Animation {
        Animation::new(Motion::new(self.exit_duration(config)).with_easing(ease_in_cubic))
    }

    /// Styles `element` at `progress` through the entrance, from 0 to 1.
    /// Overshooting curves can pass 1.
    pub(crate) fn apply<E: Styled>(&self, element: E, progress: f32, config: &MotionConfig) -> E {
        self.apply_with_offset(element, progress, config, (0.0, 0.0))
    }

    /// Styles `element` while preserving its existing relative position.
    pub(crate) fn apply_with_offset<E: Styled>(
        &self,
        mut element: E,
        progress: f32,
        config: &MotionConfig,
        base_offset: (f32, f32),
    ) -> E {
        let effect_scale = config.effect_strength.clamp(0.0, 3.0);
        let remaining = 1.0 - progress;
        if let Some(from) = self.opacity_from {
            element = element.opacity((from + (1.0 - from) * progress).clamp(0.0, 1.0));
        }
        if let Some((x, y)) = self.offset_from {
            element = element
                .left(px(base_offset.0 + x * effect_scale * remaining))
                .top(px(base_offset.1 + y * effect_scale * remaining));
        }
        if let Some(radius) = self.blur_from {
            let radius = radius * effect_scale * remaining;
            if radius > 0.0 {
                element = element.blur(px(radius));
            }
        }
        if let Some((size, fraction)) = self.grow {
            let start = (1.0 - (1.0 - fraction) * effect_scale).clamp(0.2, 1.0);
            element = element.size(px(size * (start + (1.0 - start) * progress)));
        }
        if let Some(from) = self.scale_from {
            let start = (1.0 - (1.0 - from) * effect_scale).clamp(0.2, 1.0);
            let scale = start + (1.0 - start) * progress;
            element = element.scale(scale);
        }
        element
    }
}

impl Default for Entrance {
    fn default() -> Self {
        Self::new()
    }
}

/// Whether an element is arriving or leaving.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transition {
    /// The element is appearing.
    Enter,
    /// The element is leaving.
    Exit,
}

/// Direction of an element transition; an alias for [`Transition`].
pub type MotionDirection = Transition;

impl Transition {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Enter => "enter",
            Self::Exit => "exit",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Entrance;
    use crate::MotionConfig;
    use std::time::Duration;

    #[test]
    fn entrance_and_exit_use_configured_timing() {
        let config = MotionConfig::default();
        let entrance = Entrance::new().fade_from(0.0);
        assert_eq!(entrance.total_duration(&config), Duration::from_millis(200));
        assert_eq!(entrance.exit_duration(&config), Duration::from_millis(150));
    }

    #[test]
    fn disabled_motion_removes_delays_and_exit_duration() {
        let config = MotionConfig {
            enabled: false,
            ..MotionConfig::default()
        };
        let entrance = Entrance::new()
            .fade_from(0.0)
            .delay(Duration::from_millis(80));
        assert_eq!(entrance.total_duration(&config), Duration::ZERO);
        assert_eq!(entrance.exit_duration(&config), Duration::ZERO);
    }
}
