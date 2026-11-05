use photon_core::Feature;

use super::BuiltinPlugin;
use crate::platform::ui::pages::HISTORY;

pub(super) const PLUGIN: BuiltinPlugin = BuiltinPlugin {
    pages: &[HISTORY],
    ..BuiltinPlugin::feature(Feature::History)
};
