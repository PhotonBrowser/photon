//! Semantic haptic feedback for Photon UI.

use gpui::{App, HapticFeedbackStyle};

/// The interaction represented by a haptic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feedback {
    /// A discrete control state change, such as a switch changing sides.
    StateChange,
    /// Crossing an alignment point, detent, or snap threshold.
    Alignment,
    /// A detent whose character becomes stronger with a normalized 0–100 level.
    /// AppKit offers named patterns rather than programmable amplitude.
    Detent(u8),
}

/// Plays semantic feedback using GPUI-CE's platform haptic support.
pub fn play(cx: &App, feedback: Feedback) {
    let style = match feedback {
        Feedback::StateChange => HapticFeedbackStyle::LevelChange,
        Feedback::Alignment => HapticFeedbackStyle::Alignment,
        Feedback::Detent(0..=34) => HapticFeedbackStyle::Alignment,
        Feedback::Detent(35..=69) => HapticFeedbackStyle::Generic,
        Feedback::Detent(_) => HapticFeedbackStyle::LevelChange,
    };

    cx.play_haptic_feedback(style);
}
