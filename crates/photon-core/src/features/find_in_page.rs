//! The built-in find in page feature.

use super::*;

pub(super) const DEFINITION: FeatureDefinition = feature!(
        FindInPage,
        "find-in-page",
        "Find in page",
        "Search for text in the current page.",
        PageTools,
        Registry,
        NO_REQUIREMENTS,
        &[Command::FindInPage]
    );
