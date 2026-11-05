//! The built-in animations feature.

use super::*;

pub(super) const DEFINITION: FeatureDefinition = feature!(
        Animations,
        "animations",
        "Interface animations",
        "Animate browser interface changes. Timing remains configurable in the profile.",
        Interaction,
        Animations,
        NO_REQUIREMENTS
    );
