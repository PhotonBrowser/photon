//! Browser navigation controls and full-width address toolbar.

use gpui::{App, ClickEvent, MouseButton, Role, Window, prelude::*, px, rgb, rgba};

use super::icons::{back_icon, close_icon, forward_icon, more_icon, reload_icon};
use super::layout::h_stack;
use super::{metrics, theme::ThemeColors};

pub(super) type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

pub(super) fn address_toolbar(
    omnibox: impl IntoElement,
    can_go_back: bool,
    can_go_forward: bool,
    can_reload: bool,
    loading: bool,
    menu_open: bool,
    palette: ThemeColors,
    on_back: ClickHandler,
    on_forward: ClickHandler,
    on_reload: ClickHandler,
    on_menu: ClickHandler,
) -> impl IntoElement {
    h_stack()
        .w_full()
        .h(px(metrics::TOOLBAR_HEIGHT))
        .items_center()
        .gap(px(metrics::TOOLBAR_CONTROL_GAP))
        .px(px(metrics::TOOLBAR_HORIZONTAL_INSET))
        .tab_group()
        .child(navigation_button(
            "toolbar-back",
            "Go back",
            0,
            can_go_back,
            back_icon(palette.text_secondary, metrics::TOOLBAR_ICON_SIZE),
            palette,
            on_back,
        ))
        .child(navigation_button(
            "toolbar-forward",
            "Go forward",
            1,
            can_go_forward,
            forward_icon(palette.text_secondary, metrics::TOOLBAR_ICON_SIZE),
            palette,
            on_forward,
        ))
        .child(navigation_button(
            "toolbar-reload",
            if loading {
                "Stop loading"
            } else {
                "Reload page"
            },
            2,
            can_reload || loading,
            if loading {
                close_icon(palette.text_secondary, metrics::TOOLBAR_ICON_SIZE).into_any_element()
            } else {
                reload_icon(palette.text_secondary, metrics::TOOLBAR_ICON_SIZE).into_any_element()
            },
            palette,
            on_reload,
        ))
        .child(omnibox)
        .child(menu_button(menu_open, palette, on_menu))
}

fn menu_button(menu_open: bool, palette: ThemeColors, on_click: ClickHandler) -> impl IntoElement {
    let mut button = h_stack()
        .id("toolbar-menu")
        .role(Role::Button)
        .aria_label(if menu_open {
            "Close browser menu"
        } else {
            "Open browser menu"
        })
        .aria_expanded(menu_open)
        .tab_index(4)
        .focus_visible(|style| style.border_1().border_color(rgb(palette.accent)))
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(metrics::TOOLBAR_BUTTON_SIZE))
        .rounded(px(metrics::CONTROL_RADIUS))
        .text_color(rgb(palette.text_primary))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(on_click)
        .child(more_icon(palette.text_primary, metrics::TOOLBAR_ICON_SIZE));

    button = if menu_open {
        button.bg(rgba(palette.control_hover_surface))
    } else {
        button.hover(|style| style.bg(rgba(palette.control_hover_surface)))
    };

    button
}

fn navigation_button(
    id: &'static str,
    label: &'static str,
    tab_index: isize,
    enabled: bool,
    icon: impl IntoElement,
    palette: ThemeColors,
    on_click: ClickHandler,
) -> impl IntoElement {
    let mut button = h_stack()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .aria_disabled(!enabled)
        .focus_visible(|style| style.border_1().border_color(rgb(palette.accent)))
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(metrics::TOOLBAR_BUTTON_SIZE))
        .rounded(px(metrics::CONTROL_RADIUS))
        .text_color(rgb(if enabled {
            palette.text_secondary
        } else {
            palette.text_disabled
        }))
        .child(icon);

    if enabled {
        button = button
            .tab_index(tab_index)
            .hover(|style| style.bg(rgba(palette.control_hover_surface)))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(on_click);
    }

    button
}
