//! The built-in keyboard shortcuts feature.

use super::*;

pub(super) const DEFINITION: FeatureDefinition = feature!(
        KeyboardShortcuts,
        "keyboard-shortcuts",
        "Keyboard shortcuts help",
        "Open the built-in list of browser keyboard shortcuts.",
        PageTools,
        Registry,
        NO_REQUIREMENTS
    );
