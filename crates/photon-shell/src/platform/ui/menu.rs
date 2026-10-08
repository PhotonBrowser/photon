//! Shared surface and items for browser menus.

use gpui::{App, ClickEvent, MouseButton, Role, Toggled, Window, div, prelude::*, px, rgb, rgba};

use super::icons::check_icon;
use super::layout::{h_stack, v_stack};
use super::theme::{Palette, metrics};

pub(super) type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

pub(super) fn menu_surface(content: impl IntoElement, palette: Palette) -> impl IntoElement {
    v_stack()
        .id("browser-menu")
        .role(Role::Menu)
        .aria_label("Browser menu")
        .tab_group()
        .w(px(metrics::MENU_WIDTH))
        .p(px(metrics::MENU_PADDING))
        .gap(px(metrics::MENU_ITEM_GAP))
        .rounded(px(metrics::MENU_RADIUS))
        .border_1()
        .border_color(rgba(palette.context_menu_border))
        .bg(rgba(palette.context_menu_surface))
        .text_size(px(metrics::MENU_FONT_SIZE))
        .text_color(rgb(palette.text))
        .shadow_lg()
        .child(content)
}

pub(super) fn menu_section(label: &'static str, palette: Palette) -> impl IntoElement {
    div()
        .id("browser-menu-theme-heading")
        .role(Role::Heading)
        .aria_level(3)
        .w_full()
        .px(px(metrics::MENU_SECTION_INSET))
        .py(px(metrics::MENU_ITEM_GAP))
        .text_size(px(metrics::TAB_FONT_SIZE))
        .text_color(rgb(palette.text_muted))
        .child(label)
}

pub(super) fn menu_separator(palette: Palette) -> impl IntoElement {
    div()
        .id("browser-menu-separator")
        .aria_hidden()
        .w_full()
        .h(px(metrics::MENU_SEPARATOR_HEIGHT))
        .my(px(metrics::MENU_ITEM_GAP))
        .bg(rgba(palette.context_menu_border))
}

pub(super) fn menu_action(
    id: &'static str,
    label: &'static str,
    tab_index: isize,
    on_click: ClickHandler,
    palette: Palette,
) -> impl IntoElement {
    menu_item(id, label, tab_index, Role::MenuItem, palette).on_click(on_click)
}

pub(super) fn menu_checkbox(
    id: &'static str,
    label: &'static str,
    tab_index: isize,
    checked: bool,
    on_click: ClickHandler,
    palette: Palette,
) -> impl IntoElement {
    let toggled = if checked {
        Toggled::True
    } else {
        Toggled::False
    };
    menu_item(id, label, tab_index, Role::MenuItemCheckBox, palette)
        .aria_toggled(toggled)
        .child(if checked {
            check_icon(palette.text_muted, metrics::TOOLBAR_ICON_SIZE).into_any_element()
        } else {
            div()
                .size(px(metrics::TOOLBAR_ICON_SIZE))
                .into_any_element()
        })
        .on_click(on_click)
}

pub(super) fn menu_radio(
    id: &'static str,
    label: &'static str,
    tab_index: isize,
    selected: bool,
    on_click: ClickHandler,
    palette: Palette,
) -> impl IntoElement {
    menu_item(id, label, tab_index, Role::MenuItemRadio, palette)
        .aria_selected(selected)
        .child(if selected {
            check_icon(palette.text_muted, metrics::TOOLBAR_ICON_SIZE).into_any_element()
        } else {
            div()
                .size(px(metrics::TOOLBAR_ICON_SIZE))
                .into_any_element()
        })
        .on_click(on_click)
}

fn menu_item(
    id: &'static str,
    label: &'static str,
    tab_index: isize,
    role: Role,
    palette: Palette,
) -> gpui::Stateful<gpui::Div> {
    h_stack()
        .id(id)
        .role(role)
        .aria_label(label)
        .tab_index(tab_index)
        .focus_visible(|style| style.border_1().border_color(rgb(palette.accent)))
        .items_center()
        .justify_between()
        .w_full()
        .h(px(metrics::MENU_ITEM_HEIGHT))
        .px(px(metrics::MENU_ITEM_HORIZONTAL_PADDING))
        .rounded(px(metrics::MENU_ITEM_RADIUS))
        .hover(|style| style.bg(rgba(palette.context_menu_hover)))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(label)
}
