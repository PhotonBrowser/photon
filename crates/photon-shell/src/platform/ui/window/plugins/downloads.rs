use gpui::{Context, Window};
use photon_core::Feature;

use super::{BuiltinPlugin, super::BrowserWindow};
use crate::platform::ui::pages::DOWNLOADS;

pub(super) const PLUGIN: BuiltinPlugin = BuiltinPlugin {
    pages: &[DOWNLOADS],
    on_enable: Some(enable),
    on_disable: Some(disable),
    ..BuiltinPlugin::feature(Feature::Downloads)
};

fn disable(browser: &mut BrowserWindow, _: &mut Window, cx: &mut Context<BrowserWindow>) {
    browser._downloads_subscription = None;
    browser.dismiss_downloads_popover(cx);
}

fn enable(browser: &mut BrowserWindow, _: &mut Window, cx: &mut Context<BrowserWindow>) {
    if browser._downloads_subscription.is_some() {
        return;
    }
    browser._downloads_subscription = Some(
        cx.observe_global::<super::super::super::downloads::Downloads>(
            |this: &mut BrowserWindow, cx| {
                this.chrome.update(cx, |_, cx| cx.notify());
                this.overlays.update(cx, |_, cx| cx.notify());
                cx.notify();
            },
        ),
    );
}
