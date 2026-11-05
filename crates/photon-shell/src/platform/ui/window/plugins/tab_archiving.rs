use gpui::{Context, Window};
use photon_core::Feature;

use super::{BuiltinPlugin, super::BrowserWindow};

pub(super) const PLUGIN: BuiltinPlugin = BuiltinPlugin {
    on_enable: Some(enable),
    on_disable: Some(disable),
    ..BuiltinPlugin::feature(Feature::TabArchiving)
};

fn enable(browser: &mut BrowserWindow, window: &mut Window, cx: &mut Context<BrowserWindow>) {
    browser.enable_tab_archiving(window, cx);
}

fn disable(browser: &mut BrowserWindow, _: &mut Window, _: &mut Context<BrowserWindow>) {
    browser.disable_tab_archiving();
}
