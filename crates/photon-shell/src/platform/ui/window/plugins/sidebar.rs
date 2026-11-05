use gpui::{Context, Window};
use photon_core::Feature;

use super::{BuiltinPlugin, ComponentId, super::BrowserWindow};

pub(super) const PLUGIN: BuiltinPlugin = BuiltinPlugin {
    components: &[
        ComponentId::SidebarToggle,
        ComponentId::FavouritesPanel,
        ComponentId::VerticalTabs,
        ComponentId::SidebarFooter,
    ],
    on_disable: Some(disable),
    ..BuiltinPlugin::feature(Feature::Sidebar)
};

fn disable(browser: &mut BrowserWindow, _: &mut Window, cx: &mut Context<BrowserWindow>) {
    browser.sidebar.reset_for_disabled_feature();
    browser.dismiss_space_editor();
    cx.notify();
}
