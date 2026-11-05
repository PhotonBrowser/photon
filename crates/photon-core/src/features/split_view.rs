//! The built-in split view feature.

use super::*;

pub(super) const DEFINITION: FeatureDefinition = feature!(
        SplitView,
        "split-view",
        "Split view",
        "Show two tabs side by side in one window.",
        Navigation,
        Registry,
        NO_REQUIREMENTS,
        &[Command::ToggleSplitView, Command::SwitchSplitPane]
    );
