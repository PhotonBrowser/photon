//! Always-on pages and workspace components supplied by the shell host.

use super::{BuiltinPlugin, ComponentId};
use crate::platform::ui::pages::{NEW_TAB, SETTINGS};

#[cfg(target_os = "macos")]
use crate::platform::ui::pages::APPKIT;

#[cfg(target_os = "macos")]
pub(super) const PLUGIN: BuiltinPlugin = BuiltinPlugin::core(
    &[NEW_TAB, SETTINGS, APPKIT],
    &[
        ComponentId::TabStrip,
        ComponentId::BackButton,
        ComponentId::ForwardButton,
        ComponentId::ReloadButton,
        ComponentId::AddressField,
        ComponentId::BrowserMenuButton,
    ],
);

#[cfg(not(target_os = "macos"))]
pub(super) const PLUGIN: BuiltinPlugin = BuiltinPlugin::core(
    &[NEW_TAB, SETTINGS],
    &[
        ComponentId::TabStrip,
        ComponentId::BackButton,
        ComponentId::ForwardButton,
        ComponentId::ReloadButton,
        ComponentId::AddressField,
        ComponentId::BrowserMenuButton,
    ],
);
