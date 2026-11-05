//! The built-in downloads feature.

use super::*;

pub(super) const DEFINITION: FeatureDefinition = feature!(
        Downloads,
        "downloads",
        "Downloads interface",
        "Show download controls, progress, and the downloads page.",
        PageTools,
        Registry,
        NO_REQUIREMENTS
    );
