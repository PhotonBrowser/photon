//! Shared surface and items for browser menus.

use gpui::{App, ElementId, Role, SharedString, div, prelude::*, px, rgb, rgba};

use super::controls::{switch, toggled};
use super::icons::check_icon;
use super::layout::{Elevated, Elevation, Raised};
use super::layout::{h_stack, v_stack};
use super::motion::pressable;
use super::motion::{AnimateIn, Entrance, Transition};
use super::settings::Settings;
use super::{metrics, theme::ThemeColors};

use super::ClickHandler;
use super::controls::FocusRing;
use super::controls::OwnsClicks;
use std::cell::Cell;
use std::rc::Rc;

/// How menus open and close.
pub(super) const MENU_MOTION: Entrance = Entrance::popover();

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
    cx: &App,
) -> impl IntoElement {
    v_stack()
        .id("browser-menu")
        .role(role)
        .aria_label(label)
        .tab_group()
        .w(px(width))
        // Rows sit inset from the edges, so their rounded highlight sits
        // inside the rounded surface.
        .p(px(metrics::MENU_PADDING))
        .rounded(px(metrics::SURFACE_RADIUS))
        .border_1()
        .border_color(rgba(palette.menu_border))
        .raised(palette)
        .text_size(px(metrics::MENU_FONT_SIZE))
        .text_color(rgb(palette.text_primary))
        .elevated(Elevation::High)
        .child(content)
        .animate(
            "browser-menu-motion",
            MENU_MOTION,
            transition,
            &Settings::motion(cx),
        )
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

/// A menu row that does something when chosen. Its keyboard shortcut, if
/// any, can follow its label as a child.
pub(super) fn menu_action(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    tab_index: isize,
    on_click: ClickHandler,
    palette: ThemeColors,
) -> gpui::Stateful<gpui::Div> {
    menu_item(id, label, tab_index, Role::MenuItem, palette).on_click(on_click)
}

/// An option in a list box, such as a dropdown's menu, checked while chosen.
pub(super) fn menu_option(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    chosen: bool,
    on_click: ClickHandler,
    palette: ThemeColors,
) -> impl IntoElement {
    menu_item(id, label, 0, Role::ListBoxOption, palette)
        .aria_selected(chosen)
        .child(if chosen {
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
    let on_toggle = Rc::new(on_toggle);
    let row_toggle = on_toggle.clone();
    let dragged = Rc::new(Cell::new(false));
    let row_dragged_on_down = dragged.clone();
    let row_dragged_on_click = dragged.clone();
    menu_item(id, label, tab_index, Role::MenuItemCheckBox, palette)
        .aria_toggled(toggled(on))
        .child(switch(id, on, palette, None, on_toggle, dragged))
        .on_mouse_down_all(move |event, phase, _, _, _| {
            if phase == gpui::DispatchPhase::Capture && event.button == gpui::MouseButton::Left {
                row_dragged_on_down.set(false);
            }
        })
        .on_click(move |event, window, cx| {
            if !row_dragged_on_click.replace(false) {
                row_toggle(event, window, cx);
            }
        })
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

fn menu_item(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    tab_index: isize,
    role: Role,
    palette: ThemeColors,
) -> gpui::Stateful<gpui::Div> {
    pressable(menu_row(id, label.into(), role).tab_index(tab_index))
        .focus_ring(palette)
        .rounded(px(metrics::MENU_ITEM_RADIUS))
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
        .gap(px(metrics::MENU_ITEM_GAP))
        .w_full()
        .h(px(metrics::MENU_ITEM_HEIGHT))
        .px(px(metrics::MENU_ITEM_HORIZONTAL_PADDING))
        .owns_clicks()
        // The label takes the room, so what follows it sits at the end.
        .child(div().flex_1().min_w_0().truncate().child(label))
}
