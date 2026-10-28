//! Retargetable scalars for layout that animates as a whole, such as the
//! sidebar, and the springs they follow.

use super::*;

/// Animates a vertical position offset with a retained element motion channel.
pub(in super::super) fn animate_vertical_position<E>(
    element: E,
    id: impl Into<ElementId>,
    from: f32,
    duration: Duration,
) -> gpui::AnyElement
where
    E: Styled + gpui::IntoElement + 'static,
{
    let motion = Motion::new(Target::default().y(0.0))
        .initial(Target::default().y(from))
        .transition(TweenCurve::EaseOut.transition(duration));
    MotionElement::new(id, element, motion)
        .map(|element: E, frame: Frame| element.top(gpui::px(frame.target().y_px())))
        .into_any_element()
}

/// A continuously retargetable scalar backed by the fork's analytic MotionValue.
#[derive(Clone, Debug)]
pub(in super::super) struct Tween {
    value: std::cell::RefCell<MotionValue>,
    epoch: Instant,
    target: f32,
    curve: TweenCurve,
}

impl Tween {
    pub fn at(value: f32) -> Self {
        Self::at_with_curve(value, TweenCurve::default())
    }

    pub fn at_with_curve(value: f32, curve: TweenCurve) -> Self {
        Self {
            value: std::cell::RefCell::new(MotionValue::new(f64::from(value))),
            epoch: Instant::now(),
            target: value,
            curve,
        }
    }

    pub fn animate_to(&mut self, target: f32, speed: Speed, config: &MotionConfig) {
        self.animate_to_for(target, config.duration(speed), config);
    }

    pub fn animate_to_for(&mut self, target: f32, duration: Duration, config: &MotionConfig) {
        if !config.allows_motion() {
            self.jump_to(target);
        } else {
            self.animate_to_at(target, duration, self.curve, Instant::now());
        }
    }

    pub fn animate_to_at(
        &mut self,
        target: f32,
        duration: Duration,
        curve: TweenCurve,
        now: Instant,
    ) {
        if self.target == target && !duration.is_zero() {
            return;
        }
        self.target = target;
        self.curve = curve;
        self.value.borrow_mut().animate(
            f64::from(target),
            curve.transition(duration),
            now.saturating_duration_since(self.epoch),
        );
    }

    pub fn jump_to(&mut self, value: f32) {
        self.jump_to_at(value, Instant::now());
    }

    pub fn jump_to_at(&mut self, value: f32, now: Instant) {
        self.target = value;
        self.value
            .borrow_mut()
            .jump(f64::from(value), now.saturating_duration_since(self.epoch));
    }

    pub fn value_at(&self, now: Instant) -> f32 {
        self.value
            .borrow_mut()
            .sample(now.saturating_duration_since(self.epoch), false)
            .value() as f32
    }

    #[allow(dead_code)]
    pub fn value_with_config(&self, now: Instant, config: &MotionConfig) -> f32 {
        if config.allows_motion() {
            self.value_at(now)
        } else {
            self.target
        }
    }

    pub fn value(&self) -> f32 {
        self.value_at(Instant::now())
    }

    pub fn target(&self) -> f32 {
        self.target
    }

    pub fn is_running_at(&self, now: Instant) -> bool {
        self.value
            .borrow_mut()
            .sample(now.saturating_duration_since(self.epoch), false)
            .is_animating()
    }

    #[allow(dead_code)]
    pub fn is_running_with_config(&self, now: Instant, config: &MotionConfig) -> bool {
        config.allows_motion() && self.is_running_at(now)
    }

    pub fn is_running(&self) -> bool {
        self.is_running_at(Instant::now())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[allow(dead_code)] // Matches the legacy TweenCurve options for downstream Photon UI.
pub(in super::super) enum TweenCurve {
    Linear,
    EaseOut,
    SmoothStep,
    #[default]
    EaseInOutCubic,
    Spring(SpringConfig),
}

pub(in super::super) const THEME_PICKER_SPRING: TweenCurve =
    TweenCurve::Spring(SpringConfig::new(360.0, 18.0, 1.0));

/// The sidebar opening, closing, collapsing and Zen mode tucking the chrome
/// away: critically damped, so the page beside it never overshoots, and
/// carrying its momentum when it is turned back midway.
pub(in super::super) const SIDEBAR_LAYOUT_SPRING: TweenCurve =
    TweenCurve::Spring(SpringConfig::new(520.0, 45.6, 1.0));

/// The sidebar revealed over the page: quicker and slightly underdamped, so
/// it lands with a hint of give, as in Arc.
pub(in super::super) const SIDEBAR_REVEAL_SPRING: TweenCurve =
    TweenCurve::Spring(SpringConfig::new(480.0, 38.0, 1.0));

/// A peeked link growing into its preview: underdamped, so the card swells
/// just past its frame and settles back with a soft bounce.
pub(in super::super) const PEEK_MORPH_SPRING: TweenCurve =
    TweenCurve::Spring(SpringConfig::new(340.0, 25.0, 1.0));

/// The preview shrinking back into its link: critically damped, since
/// overshooting a point would turn the card inside out.
pub(in super::super) const PEEK_CLOSE_SPRING: TweenCurve =
    TweenCurve::Spring(SpringConfig::new(420.0, 41.0, 1.0));

pub(in super::super) const SIDEBAR_RESIZE_SPRING: TweenCurve =
    TweenCurve::Spring(SpringConfig::new(520.0, 30.0, 1.0));

impl TweenCurve {
    fn transition(self, duration: Duration) -> gpui_motion::Transition {
        let easing = match self {
            Self::Linear => Easing::Linear,
            Self::EaseOut => custom_ease_out(),
            Self::SmoothStep => Easing::EaseInOut,
            Self::EaseInOutCubic => custom_ease_in_out_cubic(),
            Self::Spring(config) => {
                return gpui_motion::Transition::Spring(
                    Spring::new(
                        config.stiffness,
                        config.damping.max(f32::MIN_POSITIVE),
                        config.mass,
                    )
                    .expect("Photon spring parameters are positive"),
                );
            }
        };
        if duration.is_zero() {
            return gpui_motion::Transition::Instant;
        }
        gpui_motion::Transition::Tween(duration, easing)
    }
}
