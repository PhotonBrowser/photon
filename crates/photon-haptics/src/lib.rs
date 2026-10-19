//! Semantic haptic feedback for Photon UI.

use gpui::{App, HapticFeedbackStyle};

/// The interaction represented by a haptic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feedback {
    /// A light, generic interaction tap.
    Tap,
    /// A discrete control state change, such as a switch changing sides.
    StateChange,
    /// Crossing an alignment point, detent, or snap threshold.
    Alignment,
}

/// Plays semantic feedback using GPUI-CE's platform haptic support.
pub fn play(cx: &App, feedback: Feedback) {
    let style = match feedback {
        Feedback::Tap => HapticFeedbackStyle::Generic,
        Feedback::StateChange => HapticFeedbackStyle::LevelChange,
        Feedback::Alignment => HapticFeedbackStyle::Alignment,
    };

    cx.play_haptic_feedback(style);
}
