//! Shell plugins discovered from the modules in this directory.
//!
//! Core feature files own settings and command metadata. These shell modules
//! own each feature's views, workspace components, and window lifecycle.

use gpui::{Context, Window};
#[cfg(test)]
use photon_core::FEATURE_REGISTRY;
use photon_core::{Command, Feature, FeatureSettings};

use super::BrowserWindow;
use crate::platform::ui::pages::PageDefinition;
use crate::platform::ui::workspace_layout::ComponentId;

type LifecycleHook = fn(&mut BrowserWindow, &mut Window, &mut Context<BrowserWindow>);

/// One built-in plugin's shell contributions and lifecycle callbacks.
#[derive(Clone, Copy)]
pub(super) struct BuiltinPlugin {
    feature: Option<Feature>,
    pages: &'static [&'static PageDefinition],
    components: &'static [ComponentId],
    on_enable: Option<LifecycleHook>,
    on_disable: Option<LifecycleHook>,
}

impl BuiltinPlugin {
    /// Creates a feature plugin with no shell-specific contributions.
    pub(super) const fn feature(feature: Feature) -> Self {
        Self {
            feature: Some(feature),
            pages: &[],
            components: &[],
            on_enable: None,
            on_disable: None,
        }
    }

    /// Creates the always-on host plugin for core pages and components.
    pub(super) const fn core(
        pages: &'static [&'static PageDefinition],
        components: &'static [ComponentId],
    ) -> Self {
        Self {
            feature: None,
            pages,
            components,
            on_enable: None,
            on_disable: None,
        }
    }

    fn commands(self) -> &'static [Command] {
        self.feature
            .map(|feature| feature.definition().commands)
            .unwrap_or(&[])
    }
}

include!(concat!(env!("OUT_DIR"), "/shell_plugin_registry.rs"));

/// Runs activation or cleanup when a setting changes a plugin's availability.
pub(super) fn apply_feature_changes(
    browser: &mut BrowserWindow,
    previous: &FeatureSettings,
    features: &FeatureSettings,
    window: &mut Window,
    cx: &mut Context<BrowserWindow>,
) {
    for plugin in BUILTIN_PLUGINS {
        let Some(feature) = plugin.feature else {
            continue;
        };
        let was_enabled = previous.is_enabled(feature);
        let enabled = features.is_enabled(feature);
        if was_enabled != enabled {
            let hook = if enabled {
                plugin.on_enable
            } else {
                plugin.on_disable
            };
            if let Some(hook) = hook {
                hook(browser, window, cx);
            }
        }
    }
}

/// Pages contributed by every built-in plugin, in generated registry order.
pub(in crate::platform::ui) fn pages() -> impl Iterator<Item = &'static PageDefinition> {
    BUILTIN_PLUGINS
        .iter()
        .flat_map(|plugin| plugin.pages.iter().copied())
}

/// Whether the plugin owning `page` is enabled.
pub(in crate::platform::ui) fn page_enabled(
    page: &PageDefinition,
    features: &FeatureSettings,
) -> bool {
    BUILTIN_PLUGINS
        .iter()
        .find(|plugin| {
            plugin
                .pages
                .iter()
                .any(|registered| std::ptr::eq(*registered, page))
        })
        .is_some_and(|plugin| {
            plugin
                .feature
                .is_none_or(|feature| features.is_enabled(feature))
        })
}

/// The optional feature that owns a workspace component, if any.
pub(in crate::platform::ui) fn component_feature(id: ComponentId) -> Option<Feature> {
    BUILTIN_PLUGINS
        .iter()
        .find(|plugin| plugin.components.contains(&id))
        .and_then(|plugin| plugin.feature)
}

/// Whether a plugin declares this workspace component.
#[cfg(test)]
pub(in crate::platform::ui) fn has_component(id: ComponentId) -> bool {
    BUILTIN_PLUGINS
        .iter()
        .any(|plugin| plugin.components.contains(&id))
}

/// The feature plugin that contributes a command, if any.
pub(in crate::platform::ui) fn feature_for_command(command: Command) -> Option<Feature> {
    BUILTIN_PLUGINS
        .iter()
        .find(|plugin| plugin.commands().contains(&command))
        .and_then(|plugin| plugin.feature)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn each_registered_feature_has_one_shell_plugin() {
        let features = BUILTIN_PLUGINS
            .iter()
            .filter_map(|plugin| plugin.feature)
            .collect::<HashSet<_>>();

        assert_eq!(features.len(), BUILTIN_PLUGINS.len() - 1);
        assert_eq!(features.len(), FEATURE_REGISTRY.builtins().len());
        assert!(
            FEATURE_REGISTRY
                .builtins()
                .iter()
                .all(|definition| features.contains(&definition.feature))
        );
    }

    #[test]
    fn shell_contributions_have_unique_pages_components_and_commands() {
        let pages = pages().collect::<Vec<_>>();
        assert_eq!(
            pages
                .iter()
                .map(|page| *page as *const _)
                .collect::<HashSet<_>>()
                .len(),
            pages.len()
        );

        let components = BUILTIN_PLUGINS
            .iter()
            .flat_map(|plugin| plugin.components.iter().copied())
            .collect::<HashSet<_>>();
        let component_count = BUILTIN_PLUGINS
            .iter()
            .map(|plugin| plugin.components.len())
            .sum::<usize>();
        assert_eq!(components.len(), component_count);

        let commands = BUILTIN_PLUGINS
            .iter()
            .flat_map(|plugin| plugin.commands().iter().copied())
            .collect::<HashSet<_>>();
        let command_count = BUILTIN_PLUGINS
            .iter()
            .map(|plugin| plugin.commands().len())
            .sum::<usize>();
        assert_eq!(commands.len(), command_count);
    }
}
