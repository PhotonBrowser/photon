use gpui::{Context, Window};
use photon_core::Feature;

use super::{BuiltinPlugin, super::BrowserWindow};

pub(super) const PLUGIN: BuiltinPlugin = BuiltinPlugin {
    on_disable: Some(disable),
    ..BuiltinPlugin::feature(Feature::GlassMenus)
};

fn disable(_: &mut BrowserWindow, _: &mut Window, cx: &mut Context<BrowserWindow>) {
    super::super::super::glass::release_native_glass(cx);
}
