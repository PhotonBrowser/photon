//! Built-in feature plugins and their window lifecycle hooks.
//!
//! The core feature registry supplies stable IDs, settings metadata, and
//! dependencies. Shell registries attach pages and workspace components to
//! those IDs; this registry attaches cleanup hooks for transient window UI.

use gpui::{Context, Window};
use photon_core::{FEATURE_REGISTRY, Feature, FeatureDefinition, FeatureSettings};

use super::BrowserWindow;

type DisableHook = fn(&mut BrowserWindow, &mut Window, &mut Context<BrowserWindow>);
type EnableHook = fn(&mut BrowserWindow, &mut Window, &mut Context<BrowserWindow>);

/// One registered built-in plugin, joined with its optional shell lifecycle.
#[derive(Clone, Copy)]
struct BuiltinPlugin {
    definition: &'static FeatureDefinition,
    on_enable: Option<EnableHook>,
    on_disable: Option<DisableHook>,
}

/// The built-in plugin registry. Every optional feature from the core
/// registry is represented, including plugins without window cleanup work.
struct BuiltinPluginRegistry;

impl BuiltinPluginRegistry {
    fn plugins() -> impl Iterator<Item = BuiltinPlugin> {
        FEATURE_REGISTRY
            .builtins()
            .iter()
            .map(|definition| BuiltinPlugin {
                definition,
                on_enable: enable_hook(definition.feature),
                on_disable: disable_hook(definition.feature),
            })
    }
}

fn enable_hook(feature: Feature) -> Option<EnableHook> {
    match feature {
        Feature::TabSleeping => Some(enable_tab_sleeping),
        Feature::TabArchiving => Some(enable_tab_archiving),
        Feature::Downloads => Some(enable_downloads),
        _ => None,
    }
}

fn disable_hook(feature: Feature) -> Option<DisableHook> {
    match feature {
        Feature::Sidebar => Some(disable_sidebar),
        Feature::CommandPalette => Some(disable_command_palette),
        Feature::FindInPage => Some(disable_find_in_page),
        Feature::Downloads => Some(disable_downloads),
        Feature::PerformanceTools => Some(disable_performance_tools),
        Feature::SplitView => Some(disable_split_view),
        Feature::Spaces => Some(disable_spaces),
        Feature::KeyboardShortcuts => Some(disable_keyboard_shortcuts),
        Feature::ZenMode => Some(disable_zen_mode),
        Feature::TabSleeping => Some(disable_tab_sleeping),
        Feature::TabArchiving => Some(disable_tab_archiving),
        Feature::GlassMenus => Some(disable_glass_menus),
        Feature::BackgroundNoise => Some(disable_background_noise),
        _ => None,
    }
}

/// Removes transient UI and state belonging to disabled built-in plugins.
pub(super) fn apply_feature_changes(
    browser: &mut BrowserWindow,
    previous: &FeatureSettings,
    features: &FeatureSettings,
    window: &mut Window,
    cx: &mut Context<BrowserWindow>,
) {
    for plugin in BuiltinPluginRegistry::plugins() {
        let was_enabled = previous.is_enabled(plugin.definition.feature);
        let enabled = features.is_enabled(plugin.definition.feature);
        if was_enabled != enabled {
            if enabled {
                if let Some(on_enable) = plugin.on_enable {
                    on_enable(browser, window, cx);
                }
            } else if let Some(on_disable) = plugin.on_disable {
                on_disable(browser, window, cx);
            }
        }
    }
}

fn disable_sidebar(browser: &mut BrowserWindow, _: &mut Window, cx: &mut Context<BrowserWindow>) {
    browser.sidebar.reset_for_disabled_feature();
    browser.dismiss_space_editor();
    cx.notify();
}

fn enable_tab_sleeping(
    browser: &mut BrowserWindow,
    window: &mut Window,
    cx: &mut Context<BrowserWindow>,
) {
    browser.enable_tab_sleeping(window, cx);
}

fn disable_tab_sleeping(
    browser: &mut BrowserWindow,
    _: &mut Window,
    _: &mut Context<BrowserWindow>,
) {
    browser.disable_tab_sleeping();
}

fn enable_tab_archiving(
    browser: &mut BrowserWindow,
    window: &mut Window,
    cx: &mut Context<BrowserWindow>,
) {
    browser.enable_tab_archiving(window, cx);
}

fn disable_tab_archiving(
    browser: &mut BrowserWindow,
    _: &mut Window,
    _: &mut Context<BrowserWindow>,
) {
    browser.disable_tab_archiving();
}

fn disable_glass_menus(_: &mut BrowserWindow, _: &mut Window, cx: &mut Context<BrowserWindow>) {
    super::super::glass::release_native_glass(cx);
}

fn disable_background_noise(_: &mut BrowserWindow, _: &mut Window, _: &mut Context<BrowserWindow>) {
    super::super::background_noise::release_texture();
}

fn disable_command_palette(
    browser: &mut BrowserWindow,
    window: &mut Window,
    cx: &mut Context<BrowserWindow>,
) {
    let was_open = browser.command_palette.take().is_some();
    browser._command_palette_subscription = None;
    browser.command_palette_presence = super::super::motion::Presence::default();
    if was_open {
        window.focus(&browser.root_focus, cx);
        cx.notify();
    }
}

fn disable_find_in_page(
    browser: &mut BrowserWindow,
    window: &mut Window,
    cx: &mut Context<BrowserWindow>,
) {
    browser.close_find_bar(false, window, cx);
    browser.find_bar_presence = super::super::motion::Presence::default();
}

fn disable_downloads(browser: &mut BrowserWindow, _: &mut Window, cx: &mut Context<BrowserWindow>) {
    browser._downloads_subscription = None;
    browser.dismiss_downloads_popover(cx);
}

fn enable_downloads(browser: &mut BrowserWindow, _: &mut Window, cx: &mut Context<BrowserWindow>) {
    if browser._downloads_subscription.is_some() {
        return;
    }
    browser._downloads_subscription = Some(
        cx.observe_global::<super::super::downloads::Downloads>(|this: &mut BrowserWindow, cx| {
            this.chrome.update(cx, |_, cx| cx.notify());
            this.overlays.update(cx, |_, cx| cx.notify());
            cx.notify();
        }),
    );
}

fn disable_performance_tools(
    browser: &mut BrowserWindow,
    _: &mut Window,
    cx: &mut Context<BrowserWindow>,
) {
    for webview in browser.tabs.iter().filter_map(|tab| tab.content.webview()) {
        webview.update(cx, |view, cx| {
            view.set_performance_overlay_enabled(false);
            cx.notify();
        });
    }
}

fn disable_split_view(
    browser: &mut BrowserWindow,
    _: &mut Window,
    cx: &mut Context<BrowserWindow>,
) {
    if browser.split_view.is_some() {
        browser.close_split_view(false, cx);
    }
}

fn disable_spaces(browser: &mut BrowserWindow, _: &mut Window, _: &mut Context<BrowserWindow>) {
    browser.dismiss_space_editor();
}

fn disable_keyboard_shortcuts(
    browser: &mut BrowserWindow,
    _: &mut Window,
    cx: &mut Context<BrowserWindow>,
) {
    let was_open = browser.shortcuts_dialog.take().is_some();
    browser.shortcuts_dialog_presence = super::super::motion::Presence::default();
    if was_open {
        cx.notify();
    }
}

fn disable_zen_mode(browser: &mut BrowserWindow, _: &mut Window, cx: &mut Context<BrowserWindow>) {
    if browser.sidebar.zen() {
        browser.toggle_zen_mode(cx);
    }
}

#[cfg(test)]
mod tests {
    use super::{BuiltinPluginRegistry, FEATURE_REGISTRY};
    use std::collections::HashSet;

    #[test]
    fn every_optional_feature_is_registered_as_a_builtin_plugin_once() {
        let plugins = BuiltinPluginRegistry::plugins().collect::<Vec<_>>();
        let plugin_ids = plugins
            .iter()
            .map(|plugin| plugin.definition.key)
            .collect::<HashSet<_>>();

        assert_eq!(plugins.len(), FEATURE_REGISTRY.builtins().len());
        assert_eq!(plugin_ids.len(), plugins.len());
    }
}
