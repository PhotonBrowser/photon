//! Photon presets and accessibility policy over the reusable motion engine.
//!
//! - `entrance`: how elements appear and leave, and the shared presets.
//! - `presence`: keeping an element drawn while it leaves.
//! - `press`: pressed controls, springing values and fading surfaces.
//! - `translated`: moving an element after layout.
//! - `tween`: retargetable values for layout that animates as a whole.

use gpui::{
    A11ySubtreeBuilder, App, Bounds, Element, ElementId, FocusHandle, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, Point, SpringConfig,
    StatefulInteractiveElement, Styled, Window, rgba,
};
use gpui_motion::{
    Color, CustomEasing, Easing, Frame, Motion, MotionElement, MotionValue, PressScaleElement,
    Spring, StyleMotion, StyleProperty, StyleTarget, Target,
};
use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};

pub(super) use gpui_motion::{Axis as ReorderAxis, ReorderGroup};

mod entrance;
mod presence;
mod press;
mod translated;
mod tween;

pub(super) use entrance::*;
pub(super) use presence::*;
pub(super) use press::*;
pub(super) use translated::*;
pub(super) use tween::*;

fn custom_ease_out() -> Easing {
    static EASING: OnceLock<CustomEasing> = OnceLock::new();
    Easing::Custom(*EASING.get_or_init(|| {
        CustomEasing::new(|t| {
            let remaining = 1.0 - t;
            (1.0 - remaining.powi(5), 5.0 * remaining.powi(4))
        })
        .expect("valid quintic ease-out endpoints")
    }))
}

fn custom_ease_in_out_cubic() -> Easing {
    static EASING: OnceLock<CustomEasing> = OnceLock::new();
    Easing::Custom(*EASING.get_or_init(|| {
        CustomEasing::new(|t| {
            if t < 0.5 {
                (4.0 * t.powi(3), 12.0 * t.powi(2))
            } else {
                let remaining = -2.0 * t + 2.0;
                (1.0 - remaining.powi(3) / 2.0, 3.0 * remaining.powi(2))
            }
        })
        .expect("valid cubic ease-in-out endpoints")
    }))
}

/// Application timing and accessibility preferences used by Photon presets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct MotionConfig {
    pub enabled: bool,
    pub reduce_motion: bool,
    pub effect_strength: f32,
    pub timings: MotionTimings,
}

impl MotionConfig {
    pub const fn allows_motion(self) -> bool {
        self.enabled && !self.reduce_motion
    }

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MotionTimings {
    pub quick: Duration,
    pub standard: Duration,
    pub gentle: Duration,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Speed {
    Quick,
    Standard,
    Gentle,
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)] // Keep the full preset vocabulary available to Photon UI surfaces.
pub(super) enum Curve {
    Linear,
    EaseOut,
    EaseInOut,
    Overshoot,
    Spring(SpringConfig),
}

impl Curve {
    /// Popovers and menus: quick, with a barely visible settle.
    pub const SNAPPY: Self = Self::Spring(SpringConfig::new(520.0, 36.0, 1.0));
    /// Things that arrive from further away, such as toasts and spaces: a
    /// softer spring that still comes to rest without bouncing.
    pub const SMOOTH: Self = Self::Spring(SpringConfig::new(320.0, 32.0, 1.0));

    pub(super) fn transition(self, duration: Duration) -> gpui_motion::Transition {
        if duration.is_zero() {
            return gpui_motion::Transition::Instant;
        }
        match self {
            Self::Linear => gpui_motion::Transition::Tween(duration, Easing::Linear),
            Self::EaseOut => gpui_motion::Transition::Tween(duration, custom_ease_out()),
            Self::EaseInOut => gpui_motion::Transition::Tween(duration, Easing::EaseInOut),
            Self::Overshoot => gpui_motion::Transition::Tween(duration, Easing::BackOut),
            Self::Spring(config) => gpui_motion::Transition::Spring(
                Spring::new(
                    config.stiffness,
                    config.damping.max(f32::MIN_POSITIVE),
                    config.mass,
                )
                .expect("Photon spring parameters are positive"),
            ),
        }
    }
}

/// Converts an RGBA theme token into the motion engine's linear-light color.
pub(super) fn motion_rgba_color(token: u32) -> Color {
    let color = rgba(token);
    Color::rgba(
        f64::from(color.color.red),
        f64::from(color.color.green),
        f64::from(color.color.blue),
        f64::from(color.alpha),
    )
}

/// Pixel distances selected for Photon's visual design.
pub(super) mod distance {
    pub const NUDGE: f32 = 8.0;
    pub const SHIFT: f32 = 12.0;
    pub const TRAVEL: f32 = 32.0;
}

pub(super) fn mix(from: f32, to: f32, progress: f32) -> f32 {
    from + (to - from) * progress
}
