//! The sidebar's top row: room for the native window controls, the sidebar
//! toggle, and back, forward and reload. Empty space moves the window.

use gpui::{MouseButton, Role, prelude::*, px, rgb, rgba};

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

/// The top row. With `navigation` it shows the navigation buttons too, as in
/// the sidebar; without, it is the slim bar shown while the sidebar is hidden.
pub(in super::super) fn navigation_bar(
    navigation: Option<NavigationState>,
    actions: NavigationActions,
    palette: ThemeColors,
) -> gpui::Div {
    let (color, size) = (palette.text_secondary, metrics::TOOLBAR_ICON_SIZE);
    let mut bar = window_drag_area(
        h_stack()
            .w_full()
            .h(px(metrics::TITLEBAR_HEIGHT))
            .flex_shrink_0()
            .items_center()
            .gap(px(metrics::TOOLBAR_CONTROL_GAP))
            .pl(px(metrics::WINDOW_CONTROLS_INSET))
            .pr(px(metrics::SIDEBAR_PADDING))
            .tab_group(),
    )
    .child(button(
        "toolbar-sidebar",
        if navigation.is_some() {
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
    let Some(state) = navigation else {
        return bar;
    };
    let (reload_label, reload_icon) = if state.loading {
        ("Stop loading", close_icon(color, size).into_any_element())
    } else {
        ("Reload page", reload_icon(color, size).into_any_element())
    };
    bar = bar
        .child(gpui::div().flex_1())
        .child(button(
            "toolbar-back",
            "Go back",
            1,
            state.can_go_back,
            back_icon(color, size),
            palette,
            actions.back,
        ))
        .child(button(
            "toolbar-forward",
            "Go forward",
            2,
            state.can_go_forward,
            forward_icon(color, size),
            palette,
            actions.forward,
        ))
        .child(button(
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

/// A square icon button, dimmed and inert while disabled.
pub(super) fn button(
    id: &'static str,
    label: &'static str,
    tab_index: isize,
    enabled: bool,
    icon: impl IntoElement,
    palette: ThemeColors,
    on_click: ClickHandler,
) -> impl IntoElement {
    let button = h_stack()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .aria_disabled(!enabled)
        .focus_visible(|style| style.border_1().border_color(rgb(palette.chosen)))
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(metrics::TOOLBAR_BUTTON_SIZE))
        .rounded(px(metrics::CONTROL_RADIUS))
        .child(icon);
    if !enabled {
        return button.opacity(metrics::DISABLED_OPACITY);
    }
    button
        .tab_index(tab_index)
        .hover(|style| style.bg(rgba(palette.hover_surface)))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(on_click)
}
