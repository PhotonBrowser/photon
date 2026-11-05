//! The built-in command palette feature.

use super::*;

pub(super) const DEFINITION: FeatureDefinition = feature!(
        CommandPalette,
        "command-palette",
        "Command palette",
        "Search and run browser commands from the keyboard.",
        PageTools,
        Registry,
        NO_REQUIREMENTS
    );
