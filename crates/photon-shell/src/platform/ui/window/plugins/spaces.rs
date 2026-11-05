use gpui::{Context, Window};
use photon_core::Feature;

use super::{BuiltinPlugin, super::BrowserWindow};

pub(super) const PLUGIN: BuiltinPlugin = BuiltinPlugin {
    on_disable: Some(disable),
    ..BuiltinPlugin::feature(Feature::Spaces)
};

fn disable(browser: &mut BrowserWindow, _: &mut Window, _: &mut Context<BrowserWindow>) {
    browser.dismiss_space_editor();
}
