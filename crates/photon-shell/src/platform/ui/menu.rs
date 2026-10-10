//! Shared surface and items for browser menus.

use gpui::{ElementId, MouseButton, Role, SharedString, div, prelude::*, px, rgb, rgba};

use super::button::icon_button;
use super::controls::{switch, toggled};
use super::icons::{add_icon, check_icon, minus_icon};
use super::layout::{Elevated, Elevation, Raised};
use super::layout::{h_stack, v_stack};
use super::motion::{AnimateIn, Entrance, Transition};
use super::{metrics, theme::ThemeColors};

use super::ClickHandler;

/// How menus open and close.
pub(super) const MENU_MOTION: Entrance = Entrance::popover();

/// A menu's raised surface, at the standard menu width.
pub(super) fn menu_surface(
    content: impl IntoElement,
    transition: Transition,
    palette: ThemeColors,
) -> impl IntoElement {
    popover_surface(
        "Browser menu",
        Role::Menu,
        metrics::MENU_WIDTH,
        content,
        transition,
        palette,
    )
}

/// A raised surface for menus and small panels, `width` wide. It animates in
/// each time it opens and out as it closes, so every popover, dropdown and
/// context menu built on it does too.
pub(super) fn popover_surface(
    label: &'static str,
    role: Role,
    width: f32,
    content: impl IntoElement,
    transition: Transition,
    palette: ThemeColors,
) -> impl IntoElement {
    v_stack()
        .id("browser-menu")
        .role(role)
        .aria_label(label)
        .tab_group()
        .w(px(width))
        .py(px(metrics::MENU_PADDING))
        .rounded(px(metrics::SURFACE_RADIUS))
        .border_1()
        .border_color(rgba(palette.menu_border))
        .raised(palette)
        .text_size(px(metrics::MENU_FONT_SIZE))
        .text_color(rgb(palette.text_primary))
        .elevated(Elevation::High)
        .child(content)
        .animate("browser-menu-motion", MENU_MOTION, transition)
}

pub(super) fn menu_separator(palette: ThemeColors) -> impl IntoElement {
    div()
        .id("browser-menu-separator")
        .aria_hidden()
        .w_full()
        .h(px(metrics::MENU_SEPARATOR_HEIGHT))
        .my(px(metrics::MENU_SEPARATOR_MARGIN))
        .bg(rgba(palette.menu_border))
}

pub(super) fn menu_action(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    tab_index: isize,
    on_click: ClickHandler,
    palette: ThemeColors,
) -> impl IntoElement {
    menu_item(id, label, tab_index, Role::MenuItem, palette).on_click(on_click)
}

pub(super) fn menu_checkbox(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    tab_index: isize,
    checked: bool,
    on_click: ClickHandler,
    palette: ThemeColors,
) -> impl IntoElement {
    menu_item(id, label, tab_index, Role::MenuItemCheckBox, palette)
        .aria_toggled(toggled(checked))
        .child(if checked {
            check_icon(palette.text_secondary, metrics::TOOLBAR_ICON_SIZE).into_any_element()
        } else {
            div()
                .size(px(metrics::TOOLBAR_ICON_SIZE))
                .into_any_element()
        })
        .on_click(on_click)
}

/// A menu row with a switch that turns something on or off.
pub(super) fn menu_switch(
    id: &'static str,
    label: &'static str,
    tab_index: isize,
    on: bool,
    on_toggle: ClickHandler,
    palette: ThemeColors,
) -> impl IntoElement {
    menu_item(id, label, tab_index, Role::MenuItemCheckBox, palette)
        .aria_toggled(toggled(on))
        .child(switch(id, on, palette))
        .on_click(on_toggle)
}

/// A small heading over the rows or control that follow it.
pub(super) fn menu_heading(label: &'static str, palette: ThemeColors) -> impl IntoElement {
    div()
        .w_full()
        .px(px(metrics::MENU_ITEM_HORIZONTAL_PADDING))
        .pt(px(metrics::MENU_SEPARATOR_MARGIN))
        .pb(px(metrics::MENU_ITEM_GAP))
        .text_size(px(metrics::TAB_FONT_SIZE))
        .text_color(rgb(palette.text_secondary))
        .child(label)
}

/// Lines up a control, such as a segmented choice, with the menu's rows.
pub(super) fn menu_block(content: impl IntoElement) -> impl IntoElement {
    div()
        .w_full()
        .px(px(metrics::MENU_ITEM_HORIZONTAL_PADDING))
        .pb(px(metrics::MENU_SEPARATOR_MARGIN))
        .child(content)
}

/// A menu action that cannot be chosen right now.
pub(super) fn menu_disabled(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    palette: ThemeColors,
) -> impl IntoElement {
    menu_row(id, label.into(), Role::MenuItem)
        .aria_disabled(true)
        .text_color(rgb(palette.text_secondary))
}

fn menu_item(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    tab_index: isize,
    role: Role,
    palette: ThemeColors,
) -> gpui::Stateful<gpui::Div> {
    menu_row(id, label.into(), role)
        .tab_index(tab_index)
        .focus_visible(|style| style.border_1().border_color(rgb(palette.chosen)))
        .hover(|style| style.bg(rgba(palette.hover_surface)))
}

/// A menu row's layout, with a label that is cut short when too long.
fn menu_row(
    id: impl Into<ElementId>,
    label: SharedString,
    role: Role,
) -> gpui::Stateful<gpui::Div> {
    h_stack()
        .id(id)
        .role(role)
        .aria_label(label.clone())
        .items_center()
        .justify_between()
        .gap(px(metrics::MENU_ITEM_GAP))
        .w_full()
        .h(px(metrics::MENU_ITEM_HEIGHT))
        .px(px(metrics::MENU_ITEM_HORIZONTAL_PADDING))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(div().min_w_0().truncate().child(label))
}

/// A menu row that steps a value down or up, showing the value between the
/// steppers; clicking the value resets it.
pub(super) fn menu_stepper(
    label: &'static str,
    value: SharedString,
    on_decrease: ClickHandler,
    on_reset: ClickHandler,
    on_increase: ClickHandler,
    palette: ThemeColors,
) -> impl IntoElement {
    let icon_size = metrics::ICON_BUTTON_ICON_SIZE;
    h_stack()
        .id(label)
        .role(Role::Group)
        .aria_label(label)
        .items_center()
        .justify_between()
        .w_full()
        .h(px(metrics::MENU_ITEM_HEIGHT))
        .px(px(metrics::MENU_ITEM_HORIZONTAL_PADDING))
        .child(label)
        .child(
            h_stack()
                .items_center()
                .gap(px(metrics::MENU_ITEM_GAP))
                .child(icon_button(
                    "menu-stepper-decrease",
                    "Decrease",
                    true,
                    metrics::ICON_BUTTON_SIZE,
                    minus_icon(palette.text_primary, icon_size),
                    palette,
                    on_decrease,
                ))
                .child(
                    h_stack()
                        .id("menu-stepper-value")
                        .role(Role::Button)
                        .aria_label("Reset")
                        .tab_index(0)
                        .focus_visible(|style| style.border_1().border_color(rgb(palette.chosen)))
                        .justify_center()
                        .min_w(px(metrics::MENU_STEPPER_VALUE_WIDTH))
                        .h(px(metrics::ICON_BUTTON_SIZE))
                        .items_center()
                        .rounded(px(metrics::MENU_ITEM_RADIUS))
                        .text_color(rgb(palette.text_secondary))
                        .hover(|style| style.bg(rgba(palette.hover_surface)))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(on_reset)
                        .child(value),
                )
                .child(icon_button(
                    "menu-stepper-increase",
                    "Increase",
                    true,
                    metrics::ICON_BUTTON_SIZE,
                    add_icon(palette.text_primary, icon_size),
                    palette,
                    on_increase,
                )),
        )
}
