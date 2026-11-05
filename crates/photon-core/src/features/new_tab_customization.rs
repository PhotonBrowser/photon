//! The built-in new tab customization feature.

use super::*;

pub(super) const DEFINITION: FeatureDefinition = feature!(
        NewTabCustomization,
        "new-tab-customization",
        "New tab customization",
        "Open the panel for editing shortcuts and restoring removed sites.",
        NewTab,
        Registry,
        NO_REQUIREMENTS
    );
