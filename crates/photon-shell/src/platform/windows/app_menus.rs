//! Windows menu-bar adapter. Browser commands remain available through shared UI actions.

use crate::platform::MenuBarState;
use gpui::App;

pub(crate) fn install_app_menus(_: &mut App) {}

pub(crate) fn update_menu_bar(_: MenuBarState, _: &mut App) {}
