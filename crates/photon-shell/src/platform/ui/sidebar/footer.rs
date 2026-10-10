//! The sidebar's footer: settings at the left and the browser menu at the
//! right.

use gpui::{prelude::*, px};

use super::super::button::{menu_button, toolbar_button};
use super::super::icons::settings_icon;
use super::super::layout::h_stack;
use super::super::{ClickHandler, metrics, theme::ThemeColors};

pub(in super::super) struct FooterActions {
    pub(in super::super) settings: ClickHandler,
    pub(in super::super) menu: ClickHandler,
}

pub(in super::super) fn footer(
    menu_open: bool,
    actions: FooterActions,
    palette: ThemeColors,
) -> impl IntoElement {
    h_stack()
        .w_full()
        .h(px(metrics::SIDEBAR_FOOTER_HEIGHT))
        .flex_shrink_0()
        .items_center()
        .justify_between()
        .px(px(metrics::SIDEBAR_PADDING))
        .child(toolbar_button(
            "sidebar-settings",
            "Settings",
            0,
            true,
            settings_icon(palette.text_secondary, metrics::TOOLBAR_ICON_SIZE),
            palette,
            actions.settings,
        ))
        .child(menu_button(
            "sidebar-menu",
            1,
            menu_open,
            palette,
            actions.menu,
        ))
}
