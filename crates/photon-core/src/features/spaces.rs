//! The built-in spaces feature.

use super::*;

pub(super) const DEFINITION: FeatureDefinition = feature!(
        Spaces,
        "spaces",
        "Spaces",
        "Switch between groups of tabs in the sidebar. Requires Sidebar.",
        Navigation,
        Registry,
        SIDEBAR_REQUIREMENT
    );
