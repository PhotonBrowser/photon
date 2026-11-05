//! The built-in tab archiving feature.

use super::*;

pub(super) const DEFINITION: FeatureDefinition = feature!(
        TabArchiving,
        "tab-archiving",
        "Tab archiving",
        "Close tabs that have been unused for twelve hours.",
        TabManagement,
        TabArchiving,
        NO_REQUIREMENTS
    );
