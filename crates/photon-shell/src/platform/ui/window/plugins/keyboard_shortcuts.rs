use gpui::{Context, Window};
use photon_core::Feature;

use super::{BuiltinPlugin, super::BrowserWindow};

pub(super) const PLUGIN: BuiltinPlugin = BuiltinPlugin {
    on_disable: Some(disable),
    ..BuiltinPlugin::feature(Feature::KeyboardShortcuts)
};

fn disable(browser: &mut BrowserWindow, _: &mut Window, cx: &mut Context<BrowserWindow>) {
    let was_open = browser.shortcuts_dialog.take().is_some();
    browser.shortcuts_dialog_presence = super::super::super::motion::Presence::default();
    if was_open {
        cx.notify();
    }
}
