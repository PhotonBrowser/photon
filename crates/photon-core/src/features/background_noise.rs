//! The built-in background noise feature.

use super::*;

pub(super) const DEFINITION: FeatureDefinition = feature!(
        BackgroundNoise,
        "background-noise",
        "Background texture",
        "Show decorative texture on browser window backgrounds.",
        Appearance,
        Registry,
        NO_REQUIREMENTS
    );
