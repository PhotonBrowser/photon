use gpui::{Context, Window};
use photon_core::Feature;

use super::{BuiltinPlugin, super::BrowserWindow};

pub(super) const PLUGIN: BuiltinPlugin = BuiltinPlugin {
    on_disable: Some(disable),
    ..BuiltinPlugin::feature(Feature::FindInPage)
};

fn disable(browser: &mut BrowserWindow, window: &mut Window, cx: &mut Context<BrowserWindow>) {
    browser.close_find_bar(false, window, cx);
    browser.find_bar_presence = super::super::super::motion::Presence::default();
}
