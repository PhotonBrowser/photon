//! Reusable animation mechanics for applications built with GPUI-CE.
//!
//! `gpui-motion` provides declarative element effects, easing, spring
//! extensions, interruptible tweens, and delayed-unmount presence. Application
//! preferences are supplied through [`MotionConfig`]; application-specific
//! presets and interaction policy belong in the consuming application.
//!
//! Slides use relative offsets and leave surrounding layout intact. Keep
//! position and size decisions in the application that owns the layout.

mod config;
mod curve;
mod effects;
mod element;
mod presence;
mod spring;
mod tween;

pub use config::{MotionConfig, MotionTimings};
pub use curve::{BOUNCY_SPRING, Curve, Speed, ease_in_out_cubic, lerp, mix};
pub use effects::{Edge, Entrance, MotionDirection, Transition};
pub use element::{
    AnimateIn, PressSpec, control_left_transition, control_scale_transition,
    control_thumb_transition, pressable, scale_transition,
};
pub use presence::Presence;
pub use spring::{AnimateSpring, SpringSpec};
pub use tween::{Tween, TweenCurve};
