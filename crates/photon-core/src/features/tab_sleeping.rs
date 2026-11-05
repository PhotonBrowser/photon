//! The built-in tab sleeping feature.

use super::*;

pub(super) const DEFINITION: FeatureDefinition = feature!(
        TabSleeping,
        "tab-sleeping",
        "Tab sleeping",
        "Discard idle page processes using the mode selected under Tabs.",
        TabManagement,
        TabSleeping,
        NO_REQUIREMENTS
    );
