//! The built-in interaction sounds feature.

use super::*;

pub(super) const DEFINITION: FeatureDefinition = feature!(
        InteractionSounds,
        "interaction-sounds",
        "Interaction sounds",
        "Play subtle sounds with browser interface feedback.",
        Interaction,
        InteractionSounds,
        NO_REQUIREMENTS
    );
