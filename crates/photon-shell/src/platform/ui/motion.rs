//! Shell motion presets and re-exports for the shared GPUI animation crate.

use gpui::{SpringConfig, StatefulInteractiveElement, Styled};
use gpui_motion::{PressSpec, pressable as apply_press_effect};

pub(super) use gpui_motion::{
    AnimateIn, AnimateSpring, Curve, Edge, Entrance, MotionConfig, Presence, Speed, SpringSpec,
    Transition, Tween, TweenCurve, control_scale_transition, control_thumb_transition,
    control_thumb_transition_tracking, mix,
};

/// Photon-specific control stretch tuning.
pub(super) const CONTROL_THUMB_HELD_STRETCH_X: f32 = 1.2;
pub(super) const SLIDER_EDGE_STRETCH_MAX: f32 = 0.3;
pub(super) const SLIDER_EDGE_STRETCH_SPEED_THRESHOLD: f32 = 700.0;
pub(super) const SLIDER_EDGE_STRETCH_SPEED_RANGE: f32 = 1_500.0;

/// Applies Photon's pressed-control scale and spring response.
pub(super) fn pressable<E: StatefulInteractiveElement + Styled>(element: E) -> E {
    apply_press_effect(
        element,
        PressSpec {
            scale: 0.97,
            spring: SpringConfig::new(480.0, 44.0, 1.0),
        },
    )
}

/// Pixel distances selected for Photon's visual design.
pub(super) mod distance {
    pub const HINT: f32 = 4.0;
    pub const NUDGE: f32 = 8.0;
    pub const SHIFT: f32 = 12.0;
    pub const TRAVEL: f32 = 32.0;
}

/// Photon preset for quick fades such as chips and status hints.
pub(super) const fn fade() -> Entrance {
    Entrance::new().fade_from(0.0).speed(Speed::Quick)
}

/// Photon preset for panels and dialogs.
pub(super) const fn rise() -> Entrance {
    Entrance::new()
        .fade_from(0.0)
        .slide_from(Edge::Bottom, distance::SHIFT)
}

/// Photon preset for bars that enter from above.
pub(super) const fn fall() -> Entrance {
    Entrance::new()
        .fade_from(0.0)
        .slide_from(Edge::Top, distance::NUDGE)
}

/// Photon preset for menus and popovers.
pub(super) const fn popover() -> Entrance {
    Entrance::new()
        .fade_from(0.0)
        .slide_from(Edge::Top, distance::HINT)
        .speed(Speed::Quick)
}

/// Photon preset for sidebars and sheets.
pub(super) const fn slide_in(edge: Edge) -> Entrance {
    Entrance::new()
        .fade_from(0.0)
        .slide_from(edge, distance::TRAVEL)
        .speed(Speed::Gentle)
}

/// Photon preset for tab icons.
pub(super) const fn pop(size: f32) -> Entrance {
    Entrance::new()
        .grow_from(size, 0.35)
        .curve(Curve::Overshoot)
        .speed(Speed::Standard)
}
