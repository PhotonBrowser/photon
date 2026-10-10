//! The sidebar's top row: room for the native window controls, the sidebar
//! toggle, and back, forward and reload. Empty space moves the window.

use gpui::{prelude::*, px};

use super::super::button::toolbar_button;
use super::super::icons::{back_icon, close_icon, forward_icon, reload_icon, sidebar_icon};
use super::super::layout::h_stack;
use super::super::titlebar::window_drag_area;
use super::super::{ClickHandler, metrics, theme::ThemeColors};

/// What the navigation buttons can do for the active tab.
#[derive(Clone, Copy, Default)]
pub(in super::super) struct NavigationState {
    pub(in super::super) can_go_back: bool,
    pub(in super::super) can_go_forward: bool,
    pub(in super::super) can_reload: bool,
    pub(in super::super) loading: bool,
}

pub(in super::super) struct NavigationActions {
    pub(in super::super) toggle_sidebar: ClickHandler,
    pub(in super::super) back: ClickHandler,
    pub(in super::super) forward: ClickHandler,
    pub(in super::super) reload: ClickHandler,
}

/// The top row, the sidebar's width. While the sidebar is hidden it stays
/// in place at the left of the bar above the page.
pub(in super::super) fn navigation_bar(
    state: NavigationState,
    sidebar_visible: bool,
    actions: NavigationActions,
    palette: ThemeColors,
) -> gpui::Div {
    let (color, size) = (palette.text_secondary, metrics::TOOLBAR_ICON_SIZE);
    let mut bar = window_drag_area(
        h_stack()
            .w(px(metrics::SIDEBAR_WIDTH))
            .h(px(metrics::TITLEBAR_HEIGHT))
            .flex_shrink_0()
            .items_center()
            .gap(px(metrics::SIDEBAR_CONTROL_GAP))
            .pl(px(metrics::WINDOW_CONTROLS_INSET))
            .pr(px(metrics::SIDEBAR_PADDING))
            .tab_group(),
    )
    .child(toolbar_button(
        "toolbar-sidebar",
        if sidebar_visible {
            "Hide sidebar"
        } else {
            "Show sidebar"
        },
        0,
        true,
        sidebar_icon(color, size),
        palette,
        actions.toggle_sidebar,
    ));
    let (reload_label, reload_icon) = if state.loading {
        ("Stop loading", close_icon(color, size).into_any_element())
    } else {
        ("Reload page", reload_icon(color, size).into_any_element())
    };
    bar = bar
        .child(gpui::div().flex_1())
        .child(toolbar_button(
            "toolbar-back",
            "Go back",
            1,
            state.can_go_back,
            back_icon(color, size),
            palette,
            actions.back,
        ))
        .child(toolbar_button(
            "toolbar-forward",
            "Go forward",
            2,
            state.can_go_forward,
            forward_icon(color, size),
            palette,
            actions.forward,
        ))
        .child(toolbar_button(
            "toolbar-reload",
            reload_label,
            3,
            state.can_reload || state.loading,
            reload_icon,
            palette,
            actions.reload,
        ));
    bar
}
