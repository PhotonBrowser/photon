//! The tabs section: what closing the last tab does.

use gpui::{App, prelude::*, px};

use super::super::super::layout::v_stack;
use super::super::super::settings::Settings;
use super::super::super::{metrics, theme::ThemeColors};
use super::super::change;
use super::super::controls::switch_row;
use super::super::layout::{group, secondary_text};

pub(super) fn settings(palette: ThemeColors, cx: &App) -> impl IntoElement {
    let close_window = Settings::get(cx).close_window_with_last_tab;
    v_stack()
        .gap(px(metrics::MENU_ITEM_GAP))
        .child(group(palette).child(switch_row(
            "settings-close-window-with-last-tab",
            "Close window with last tab",
            close_window,
            palette,
            change(move |settings| settings.close_window_with_last_tab = !close_window),
        )))
        .child(secondary_text(
            "When off, closing the last tab leaves the window open on a new tab page.",
            palette,
        ))
}
