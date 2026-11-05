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
    /// Native Liquid Glass or platform material behind the window.
    LiquidGlass,
    /// Native glass behind menus and popovers where supported.
    GlassMenus,
    /// Translucent browser surfaces.
    Transparency,
    /// Decorative noise on the browser's window background.
    BackgroundNoise,
    /// Transitions and other animated interface effects.
    Animations,
    /// Elastic scrolling for browser interface surfaces.
    ElasticScrolling,
    /// Sound effects for interface feedback.
    InteractionSounds,
    /// Asking sites to use dark appearance where supported.
    ForceDarkMode,
    /// The logo on the new tab page.
    NewTabLogo,
    /// Website shortcuts on the new tab page.
    NewTabShortcuts,
    /// Automatic discarding of idle page processes.
    TabSleeping,
    /// Closing tabs that have been unused for twelve hours.
    TabArchiving,
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
        FeatureDefinition {
            feature: Feature::$feature,
            key: $key,
            setting_id: concat!("settings-feature-", $key),
            name: $name,
            description: $description,
            category: FeatureCategory::$category,
            preference: FeaturePreference::$preference,
            requires: $requires,
        }
    };
}

const FEATURES: &[FeatureDefinition] = &[
    feature!(
        Sidebar,
        "sidebar",
        "Sidebar",
        "Vertical tabs, favourites, and sidebar navigation.",
        Navigation,
        Registry,
        NO_REQUIREMENTS
    ),
    feature!(
        Spaces,
        "spaces",
        "Spaces",
        "Switch between groups of tabs in the sidebar. Requires Sidebar.",
        Navigation,
        Registry,
        SIDEBAR_REQUIREMENT
    ),
    feature!(
        SplitView,
        "split-view",
        "Split view",
        "Show two tabs side by side in one window.",
        Navigation,
        Registry,
        NO_REQUIREMENTS
    ),
    feature!(
        ZenMode,
        "zen-mode",
        "Zen mode",
        "Hide the browser controls around the page for focused browsing.",
        Navigation,
        Registry,
        NO_REQUIREMENTS
    ),
    feature!(
        CommandPalette,
        "command-palette",
        "Command palette",
        "Search and run browser commands from the keyboard.",
        PageTools,
        Registry,
        NO_REQUIREMENTS
    ),
    feature!(
        FindInPage,
        "find-in-page",
        "Find in page",
        "Search for text in the current page.",
        PageTools,
        Registry,
        NO_REQUIREMENTS
    ),
    feature!(
        History,
        "history",
        "History page",
        "Browse, search, and import visited pages.",
        PageTools,
        Registry,
        NO_REQUIREMENTS
    ),
    feature!(
        Downloads,
        "downloads",
        "Downloads interface",
        "Show download controls, progress, and the downloads page.",
        PageTools,
        Registry,
        NO_REQUIREMENTS
    ),
    feature!(
        PerformanceTools,
        "performance-tools",
        "Performance tools",
        "Open live process, resource, and performance details.",
        PageTools,
        Registry,
        NO_REQUIREMENTS
    ),
    feature!(
        KeyboardShortcuts,
        "keyboard-shortcuts",
        "Keyboard shortcuts help",
        "Open the built-in list of browser keyboard shortcuts.",
        PageTools,
        Registry,
        NO_REQUIREMENTS
    ),
    feature!(
        LiquidGlass,
        "liquid-glass",
        "Liquid Glass window material",
        "Allow Liquid Glass or the platform's equivalent native window material.",
        Appearance,
        Registry,
        NO_REQUIREMENTS
    ),
    feature!(
        GlassMenus,
        "glass-menus",
        "Glass menus and popovers",
        "Use Liquid Glass behind menus and popovers where supported.",
        Appearance,
        GlassMenus,
        NO_REQUIREMENTS
    ),
    feature!(
        Transparency,
        "transparency",
        "Translucent surfaces",
        "Allow desktop translucency; choose its strength in Appearance.",
        Appearance,
        Registry,
        NO_REQUIREMENTS
    ),
    feature!(
        BackgroundNoise,
        "background-noise",
        "Background texture",
        "Show decorative texture on browser window backgrounds.",
        Appearance,
        Registry,
        NO_REQUIREMENTS
    ),
    feature!(
        ForceDarkMode,
        "force-dark-mode",
        "Force dark page appearance",
        "Ask sites to use dark themes and darken light content where supported.",
        Appearance,
        ForceDarkMode,
        NO_REQUIREMENTS
    ),
    feature!(
        Animations,
        "animations",
        "Interface animations",
        "Animate browser interface changes. Timing remains configurable in the profile.",
        Interaction,
        Animations,
        NO_REQUIREMENTS
    ),
    feature!(
        ElasticScrolling,
        "elastic-scrolling",
        "Elastic scrolling",
        "Allow browser interface surfaces to stretch past their scroll limits where supported.",
        Interaction,
        ElasticScrolling,
        NO_REQUIREMENTS
    ),
    feature!(
        InteractionSounds,
        "interaction-sounds",
        "Interaction sounds",
        "Play subtle sounds with browser interface feedback.",
        Interaction,
        InteractionSounds,
        NO_REQUIREMENTS
    ),
    feature!(
        NewTabLogo,
        "new-tab-logo",
        "New tab logo",
        "Show the Photon logo on the new tab page.",
        NewTab,
        NewTabLogo,
        NO_REQUIREMENTS
    ),
    feature!(
        NewTabShortcuts,
        "new-tab-shortcuts",
        "New tab shortcuts",
        "Show pinned and frequently visited websites on the new tab page.",
        NewTab,
        NewTabShortcuts,
        NO_REQUIREMENTS
    ),
    feature!(
        TabSleeping,
        "tab-sleeping",
        "Tab sleeping",
        "Discard idle page processes using the mode selected under Tabs.",
        TabManagement,
        TabSleeping,
        NO_REQUIREMENTS
    ),
    feature!(
        TabArchiving,
        "tab-archiving",
        "Tab archiving",
        "Close tabs that have been unused for twelve hours.",
        TabManagement,
        TabArchiving,
        NO_REQUIREMENTS
    ),
];

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
    fn registry_has_one_definition_for_every_feature() {
        let definitions = FEATURE_REGISTRY.builtins();
        for definition in definitions {
            assert_eq!(FEATURE_REGISTRY.definition(definition.feature), definition);
        }
        assert_eq!(definitions.len(), 22);
        for category in FeatureCategory::ALL {
            assert!(
                definitions
                    .iter()
                    .any(|definition| definition.category == category)
            );
        }
    }
}
