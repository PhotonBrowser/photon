//! The built-in force dark mode feature.

use super::*;

pub(super) const DEFINITION: FeatureDefinition = feature!(
        ForceDarkMode,
        "force-dark-mode",
        "Force dark page appearance",
        "Ask sites to use dark themes and darken light content where supported.",
        Appearance,
        ForceDarkMode,
        NO_REQUIREMENTS
    );
