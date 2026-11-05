//! Optional browser features and the registry generated from their modules.

use crate::Command;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeSet;

/// An optional built-in feature of the browser shell.
///
/// The build script creates one associated constant per module under
/// `src/features`, so adding a feature module also registers its stable ID.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
#[non_exhaustive]
pub struct Feature(&'static str);

impl Feature {
    /// The stable key stored in profile settings.
    pub const fn key(self) -> &'static str {
        self.0
    }
}

impl<'de> Deserialize<'de> for Feature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let key = String::deserialize(deserializer)?;
        FEATURE_REGISTRY
            .builtins()
            .iter()
            .find(|definition| definition.key == key)
            .map(|definition| definition.feature)
            .ok_or_else(|| {
                <D::Error as serde::de::Error>::custom(format!("unknown feature key `{key}`"))
            })
    }
}

/// A group of related optional features in the settings catalog.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeatureCategory {
    Navigation,
    PageTools,
    Appearance,
    Interaction,
    NewTab,
    TabManagement,
}

impl FeatureCategory {
    /// Every feature group in display order.
    pub const ALL: [Self; 6] = [
        Self::Navigation,
        Self::PageTools,
        Self::Appearance,
        Self::Interaction,
        Self::NewTab,
        Self::TabManagement,
    ];

    /// The label shown above this group in settings.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Navigation => "Navigation and layout",
            Self::PageTools => "Page tools",
            Self::Appearance => "Appearance",
            Self::Interaction => "Interface effects",
            Self::NewTab => "New tab page",
            Self::TabManagement => "Tab management",
        }
    }
}

/// The profile setting that controls one feature.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeaturePreference {
    /// A feature-specific entry in [`FeatureSettings`].
    Registry,
    /// Existing profile preference for glass menus and popovers.
    GlassMenus,
    /// Existing profile preference for interface sounds.
    InteractionSounds,
    /// Existing profile preference for dark page appearance.
    ForceDarkMode,
    /// Existing profile preference for interface animations.
    Animations,
    /// Existing profile preference for elastic scrolling.
    ElasticScrolling,
    /// Existing profile preference for the new tab logo.
    NewTabLogo,
    /// Existing profile preference for new tab shortcuts.
    NewTabShortcuts,
    /// Existing profile preference for tab sleeping.
    TabSleeping,
    /// Existing profile preference for tab archiving.
    TabArchiving,
}

/// User-facing information and requirements for one built-in feature.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FeatureDefinition {
    /// The feature this metadata describes.
    pub feature: Feature,
    /// Stable identifier saved in the profile's settings file.
    pub key: &'static str,
    /// Stable identifier for the feature's settings control.
    pub setting_id: &'static str,
    /// Label shown in settings.
    pub name: &'static str,
    /// Short description shown below the settings control.
    pub description: &'static str,
    /// Group in the Features settings page.
    pub category: FeatureCategory,
    /// The profile setting that controls this feature.
    pub preference: FeaturePreference,
    /// Features that must also be enabled for this feature to work.
    pub requires: &'static [Feature],
    /// Core commands contributed by this feature.
    pub commands: &'static [Command],
}

/// Preferences for optional features. Unlisted features remain enabled.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct FeatureSettings {
    disabled: BTreeSet<String>,
}

/// Central catalog of optional browser features.
#[derive(Clone, Copy, Debug, Default)]
pub struct FeatureRegistry;

const NO_REQUIREMENTS: &[Feature] = &[];
const SIDEBAR_REQUIREMENT: &[Feature] = &[Feature::Sidebar];

macro_rules! feature {
    ($feature:ident, $key:literal, $name:literal, $description:literal, $category:ident, $preference:ident, $requires:ident) => {
        feature!(
            $feature,
            $key,
            $name,
            $description,
            $category,
            $preference,
            $requires,
            &[]
        )
    };
    ($feature:ident, $key:literal, $name:literal, $description:literal, $category:ident, $preference:ident, $requires:ident, $commands:expr) => {
        FeatureDefinition {
            feature: Feature::$feature,
            key: $key,
            setting_id: concat!("settings-feature-", $key),
            name: $name,
            description: $description,
            category: FeatureCategory::$category,
            preference: FeaturePreference::$preference,
            requires: $requires,
            commands: $commands,
        }
    };
}

include!(concat!(env!("OUT_DIR"), "/feature_registry.rs"));

impl FeatureRegistry {
    /// Every optional feature in alphabetical, build-generated order.
    pub const fn builtins(self) -> &'static [FeatureDefinition] {
        FEATURES
    }

    /// The definition registered for `feature`.
    pub fn definition(self, feature: Feature) -> &'static FeatureDefinition {
        FEATURES
            .iter()
            .find(|definition| definition.feature == feature)
            .expect("every built-in feature must have a registry definition")
    }
}

/// The process-wide catalog of optional browser features.
pub const FEATURE_REGISTRY: FeatureRegistry = FeatureRegistry;

impl Feature {
    /// Returns the user-facing definition of this feature.
    pub fn definition(self) -> &'static FeatureDefinition {
        FEATURE_REGISTRY.definition(self)
    }
}

impl FeatureSettings {
    /// Whether this feature is enabled after applying its requirements.
    pub fn is_enabled(&self, feature: Feature) -> bool {
        !self.disabled.contains(feature.definition().key)
            && feature
                .definition()
                .requires
                .iter()
                .all(|requirement| self.is_enabled(*requirement))
    }

    /// Whether the user has enabled this feature in the registry, regardless of requirements.
    pub fn is_configured_enabled(&self, feature: Feature) -> bool {
        !self.disabled.contains(feature.definition().key)
    }

    /// Enables `feature` in the registry.
    pub fn enable(&mut self, feature: Feature) {
        self.disabled.remove(feature.definition().key);
    }

    /// Disables `feature` in the registry.
    pub fn disable(&mut self, feature: Feature) {
        self.disabled.insert(feature.definition().key.to_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_preferences_round_trip_and_missing_preferences_default_on() {
        let older: FeatureSettings = serde_json::from_str("{}").expect("valid empty settings");
        assert!(older.is_enabled(Feature::Sidebar));

        let mut settings = older;
        settings.disable(Feature::Downloads);
        let saved = serde_json::to_string(&settings).expect("serializable feature settings");
        let restored: FeatureSettings =
            serde_json::from_str(&saved).expect("valid saved feature settings");
        assert!(!restored.is_enabled(Feature::Downloads));
        assert!(restored.is_enabled(Feature::History));
    }

    #[test]
    fn unknown_disabled_feature_keys_survive_load_and_save() {
        let settings: FeatureSettings = serde_json::from_str(r#"{"disabled":["future-feature"]}"#)
            .expect("unknown feature keys are preserved");
        let saved = serde_json::to_string(&settings).expect("serializable feature settings");
        assert!(saved.contains("future-feature"));
    }

    #[test]
    fn dependent_features_require_their_parent() {
        let mut settings = FeatureSettings::default();
        settings.disable(Feature::Sidebar);
        assert!(!settings.is_enabled(Feature::Spaces));
        assert!(settings.is_configured_enabled(Feature::Spaces));

        settings.enable(Feature::Sidebar);
        assert!(settings.is_enabled(Feature::Spaces));
    }

    #[test]
    fn feature_modules_register_every_definition_once() {
        let definitions = FEATURE_REGISTRY.builtins();
        let mut keys = BTreeSet::new();
        for definition in definitions {
            assert_eq!(definition.key, definition.feature.key());
            assert!(keys.insert(definition.key));
            assert_eq!(FEATURE_REGISTRY.definition(definition.feature), definition);
        }
        assert!(!definitions.is_empty());
        for category in FeatureCategory::ALL {
            assert!(
                definitions
                    .iter()
                    .any(|definition| definition.category == category)
            );
        }
    }

    #[test]
    fn feature_serialization_uses_its_stable_key() {
        assert_eq!(
            serde_json::to_string(&Feature::NewTabLogo).unwrap(),
            "\"new-tab-logo\""
        );
        assert_eq!(
            serde_json::from_str::<Feature>("\"new-tab-logo\"").unwrap(),
            Feature::NewTabLogo
        );
        assert!(serde_json::from_str::<Feature>("\"unknown-feature\"").is_err());
    }
}
