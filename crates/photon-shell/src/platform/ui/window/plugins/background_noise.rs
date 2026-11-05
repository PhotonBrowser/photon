use gpui::{Context, Window};
use photon_core::Feature;

use super::{BuiltinPlugin, super::BrowserWindow};

pub(super) const PLUGIN: BuiltinPlugin = BuiltinPlugin {
    on_disable: Some(disable),
    ..BuiltinPlugin::feature(Feature::BackgroundNoise)
};

fn disable(_: &mut BrowserWindow, _: &mut Window, _: &mut Context<BrowserWindow>) {
    super::super::super::background_noise::release_texture();
}
