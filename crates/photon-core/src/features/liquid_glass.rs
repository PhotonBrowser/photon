//! The built-in liquid glass feature.

use super::*;

pub(super) const DEFINITION: FeatureDefinition = feature!(
        LiquidGlass,
        "liquid-glass",
        "Liquid Glass window material",
        "Allow Liquid Glass or the platform's equivalent native window material.",
        Appearance,
        Registry,
        NO_REQUIREMENTS
    );
