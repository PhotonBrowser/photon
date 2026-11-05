use gpui::{Context, Window};
use photon_core::Feature;

use super::{BuiltinPlugin, super::BrowserWindow};

pub(super) const PLUGIN: BuiltinPlugin = BuiltinPlugin {
    on_disable: Some(disable),
    ..BuiltinPlugin::feature(Feature::SplitView)
};

fn disable(browser: &mut BrowserWindow, _: &mut Window, cx: &mut Context<BrowserWindow>) {
    if browser.split_view.is_some() {
        browser.close_split_view(false, cx);
    }
}
