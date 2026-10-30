//! Windows console-termination adapter. GPUI's own window and session-end
//! handling already routes shutdown through app-quit hooks.

use gpui::App;

pub(crate) fn install() {}

pub(crate) fn quit_when_requested(_: &mut App) {}
