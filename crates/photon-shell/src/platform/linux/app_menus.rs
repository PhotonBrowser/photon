//! Linux has no platform application menu; browser commands are shown in the
//! shared GPUI menu within each window.

use crate::platform::MenuBarState;
use gpui::App;

pub(crate) fn install_app_menus(_: &mut App) {}

pub(crate) fn update_menu_bar(_: MenuBarState, _: &mut App) {}
