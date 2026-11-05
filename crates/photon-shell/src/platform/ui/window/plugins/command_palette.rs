use gpui::{Context, Window};
use photon_core::Feature;

use super::{BuiltinPlugin, super::BrowserWindow};

pub(super) const PLUGIN: BuiltinPlugin = BuiltinPlugin {
    on_disable: Some(disable),
    ..BuiltinPlugin::feature(Feature::CommandPalette)
};

fn disable(browser: &mut BrowserWindow, window: &mut Window, cx: &mut Context<BrowserWindow>) {
    let was_open = browser.command_palette.take().is_some();
    browser._command_palette_subscription = None;
    browser.command_palette_presence = super::super::super::motion::Presence::default();
    if was_open {
        window.focus(&browser.root_focus, cx);
        cx.notify();
    }
}
