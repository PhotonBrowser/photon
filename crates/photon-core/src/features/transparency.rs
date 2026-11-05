//! The built-in transparency feature.

use super::*;

pub(super) const DEFINITION: FeatureDefinition = feature!(
        Transparency,
        "transparency",
        "Translucent surfaces",
        "Allow desktop translucency; choose its strength in Appearance.",
        Appearance,
        Registry,
        NO_REQUIREMENTS
    );
