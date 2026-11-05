//! The built-in new tab shortcuts feature.

use super::*;

pub(super) const DEFINITION: FeatureDefinition = feature!(
        NewTabShortcuts,
        "new-tab-shortcuts",
        "New tab shortcuts",
        "Show pinned and frequently visited websites on the new tab page.",
        NewTab,
        NewTabShortcuts,
        NO_REQUIREMENTS
    );
