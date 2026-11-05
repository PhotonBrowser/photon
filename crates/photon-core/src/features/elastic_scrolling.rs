//! The built-in elastic scrolling feature.

use super::*;

pub(super) const DEFINITION: FeatureDefinition = feature!(
        ElasticScrolling,
        "elastic-scrolling",
        "Elastic scrolling",
        "Allow browser interface surfaces to stretch past their scroll limits where supported.",
        Interaction,
        ElasticScrolling,
        NO_REQUIREMENTS
    );
