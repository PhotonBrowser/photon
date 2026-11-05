//! The built-in zen mode feature.

use super::*;

pub(super) const DEFINITION: FeatureDefinition = feature!(
        ZenMode,
        "zen-mode",
        "Zen mode",
        "Hide the browser controls around the page for focused browsing.",
        Navigation,
        Registry,
        NO_REQUIREMENTS
    );
