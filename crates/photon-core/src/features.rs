//! Optional browser features and the stable catalog users can configure.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// An optional built-in feature of the browser shell.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[non_exhaustive]
#[serde(rename_all = "kebab-case")]
pub enum Feature {
    /// The vertical navigation and tab sidebar.
    Sidebar,
    /// Searchable command execution from the keyboard.
    CommandPalette,
    /// Page text search and its find bar.
    FindInPage,
    /// The built-in browser history view.
    History,
    /// Download progress and the built-in downloads view.
    Downloads,
    /// Live resource and process inspection tools.
    PerformanceTools,
    /// Two tabs displayed beside one another.
    SplitView,
    /// User-created groups of tabs.
    Spaces,
    /// The built-in keyboard shortcut reference.
    KeyboardShortcuts,
    /// Hiding the browser's surrounding chrome for focused browsing.
    ZenMode,
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
    /// Features that must also be enabled for this feature to work.
    pub requires: &'static [Feature],
}

/// The central catalog of optional features available in this build.
#[derive(Clone, Copy, Debug, Default)]
pub struct FeatureRegistry;

const NO_REQUIREMENTS: &[Feature] = &[];
const SIDEBAR_REQUIREMENT: &[Feature] = &[Feature::Sidebar];

const FEATURES: &[FeatureDefinition] = &[
    FeatureDefinition {
        feature: Feature::Sidebar,
        key: "sidebar",
        setting_id: "settings-feature-sidebar",
        name: "Sidebar",
        description: "Vertical tabs, favourites, and sidebar navigation.",
        requires: NO_REQUIREMENTS,
    },
    FeatureDefinition {
        feature: Feature::CommandPalette,
        key: "command-palette",
        setting_id: "settings-feature-command-palette",
        name: "Command palette",
        description: "Search and run browser commands from the keyboard.",
        requires: NO_REQUIREMENTS,
    },
    FeatureDefinition {
        feature: Feature::FindInPage,
        key: "find-in-page",
        setting_id: "settings-feature-find-in-page",
        name: "Find in page",
        description: "Search for text in the current page.",
        requires: NO_REQUIREMENTS,
    },
    FeatureDefinition {
        feature: Feature::History,
        key: "history",
        setting_id: "settings-feature-history",
        name: "History page",
        description: "Browse, search, and import visited pages.",
        requires: NO_REQUIREMENTS,
    },
    FeatureDefinition {
        feature: Feature::Downloads,
        key: "downloads",
        setting_id: "settings-feature-downloads",
        name: "Downloads interface",
        description: "Show download controls, progress, and the downloads page.",
        requires: NO_REQUIREMENTS,
    },
    FeatureDefinition {
        feature: Feature::PerformanceTools,
        key: "performance-tools",
        setting_id: "settings-feature-performance-tools",
        name: "Performance tools",
        description: "Open live process, resource, and performance details.",
        requires: NO_REQUIREMENTS,
    },
    FeatureDefinition {
        feature: Feature::SplitView,
        key: "split-view",
        setting_id: "settings-feature-split-view",
        name: "Split view",
        description: "Show two tabs side by side in one window.",
        requires: NO_REQUIREMENTS,
    },
    FeatureDefinition {
        feature: Feature::Spaces,
        key: "spaces",
        setting_id: "settings-feature-spaces",
        name: "Spaces",
        description: "Switch between groups of tabs in the sidebar. Requires Sidebar.",
        requires: SIDEBAR_REQUIREMENT,
    },
    FeatureDefinition {
        feature: Feature::KeyboardShortcuts,
        key: "keyboard-shortcuts",
        setting_id: "settings-feature-keyboard-shortcuts",
        name: "Keyboard shortcuts help",
        description: "Open the built-in list of browser keyboard shortcuts.",
        requires: NO_REQUIREMENTS,
    },
    FeatureDefinition {
        feature: Feature::ZenMode,
        key: "zen-mode",
        setting_id: "settings-feature-zen-mode",
        name: "Zen mode",
        description: "Hide the browser controls around the page for focused browsing.",
        requires: NO_REQUIREMENTS,
    },
];

/// Preferences for optional features. Unlisted features remain enabled.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct FeatureSettings {
    disabled: BTreeSet<String>,
}

impl FeatureRegistry {
    /// Every optional feature in stable display order.
    pub const fn builtins(self) -> &'static [FeatureDefinition] {
        FEATURES
    }

    /// The definition registered for `feature`.
    pub fn definition(self, feature: Feature) -> &'static FeatureDefinition {
        FEATURES
            .iter()
            .find(|definition| definition.feature == feature)
            .expect("every optional feature must have a registry definition")
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

    /// Whether the user has enabled this feature in settings, regardless of requirements.
    pub fn is_configured_enabled(&self, feature: Feature) -> bool {
        !self.disabled.contains(feature.definition().key)
    }

    /// Enables `feature` in settings.
    pub fn enable(&mut self, feature: Feature) {
        self.disabled.remove(feature.definition().key);
    }

    /// Disables `feature` in settings.
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
    fn registry_has_one_definition_for_every_feature() {
        let definitions = FEATURE_REGISTRY.builtins();
        for definition in definitions {
            assert_eq!(FEATURE_REGISTRY.definition(definition.feature), definition);
        }
        assert_eq!(definitions.len(), 10);
    }
}
