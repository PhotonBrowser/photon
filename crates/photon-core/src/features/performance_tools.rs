//! The built-in performance tools feature.

use super::*;

pub(super) const DEFINITION: FeatureDefinition = feature!(
        PerformanceTools,
        "performance-tools",
        "Performance tools",
        "Open live process, resource, and performance details.",
        PageTools,
        Registry,
        NO_REQUIREMENTS
    );
