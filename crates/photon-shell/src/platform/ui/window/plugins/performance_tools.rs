use gpui::{Context, Window};
use photon_core::Feature;

use super::{BuiltinPlugin, super::BrowserWindow};
use crate::platform::ui::pages::PERFORMANCE;

pub(super) const PLUGIN: BuiltinPlugin = BuiltinPlugin {
    pages: &[PERFORMANCE],
    on_disable: Some(disable),
    ..BuiltinPlugin::feature(Feature::PerformanceTools)
};

fn disable(browser: &mut BrowserWindow, _: &mut Window, cx: &mut Context<BrowserWindow>) {
    for webview in browser.tabs.iter().filter_map(|tab| tab.content.webview()) {
        webview.update(cx, |view, cx| {
            view.set_performance_overlay_enabled(false);
            cx.notify();
        });
    }
}
