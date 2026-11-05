//! The built-in history feature.

use super::*;

pub(super) const DEFINITION: FeatureDefinition = feature!(
        History,
        "history",
        "History page",
        "Browse, search, and import visited pages.",
        PageTools,
        Registry,
        NO_REQUIREMENTS,
        &[Command::OpenHistory]
    );
