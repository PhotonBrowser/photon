//! Settings rows for Photon's own pages: a label with a switch or checkbox,
//! the whole row clickable.

use gpui::{
    ElementId, Role, SharedString, Stateful, StatefulInteractiveElement, prelude::*, px, rgb, rgba,
};

use super::super::controls::FocusRing;
use super::super::controls::{OwnsClicks, Pressable};
use super::super::controls::{check_mark, switch, toggled};
use super::super::layout::h_stack;
use super::super::{ClickHandler, metrics, theme::ThemeColors};
use std::cell::Cell;
use std::rc::Rc;

/// A labelled on/off switch.
pub(super) fn switch_row(
    id: &'static str,
    label: &'static str,
    on: bool,
    palette: ThemeColors,
    on_toggle: ClickHandler,
) -> impl IntoElement {
    let on_toggle = Rc::new(on_toggle);
    let row_toggle = on_toggle.clone();
    let dragged = Rc::new(Cell::new(false));
    let row_dragged_on_down = dragged.clone();
    let row_dragged_on_click = dragged.clone();
    h_stack()
        .id(ElementId::Name(format!("{id}-row").into()))
        .items_center()
        .justify_between()
        .gap(px(metrics::INTERNAL_PAGE_GROUP_GAP))
        .w_full()
        .h(px(metrics::SETTINGS_ROW_HEIGHT))
        .px(px(metrics::MENU_ITEM_HORIZONTAL_PADDING))
        .rounded(px(metrics::CONTROL_RADIUS))
        .text_size(px(metrics::INTERNAL_PAGE_BODY_SIZE))
        .text_color(rgb(palette.text_primary))
        .owns_clicks()
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
        .child(
            gpui::div()
                .id(ElementId::Name(format!("{id}-label").into()))
                .aria_hidden()
                .child(label),
        )
        .child(switch(id, on, palette, Some(label), on_toggle, dragged))
}

/// A label with a control that is not part of the row, such as a button.
pub(super) fn control_row(label: &'static str, control: impl IntoElement) -> impl IntoElement {
    h_stack()
        .items_center()
        .justify_between()
        .gap(px(metrics::INTERNAL_PAGE_GROUP_GAP))
        .w_full()
        .h(px(metrics::SETTINGS_ROW_HEIGHT))
        .px(px(metrics::MENU_ITEM_HORIZONTAL_PADDING))
        .text_size(px(metrics::INTERNAL_PAGE_BODY_SIZE))
        .child(label)
        .child(control)
}

/// A labelled checkbox.
pub(super) fn checkbox_row(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    checked: bool,
    palette: ThemeColors,
    on_toggle: ClickHandler,
) -> impl IntoElement {
    let id = id.into();
    let mark = check_mark(id.clone(), checked, palette);
    row(id, label, Role::CheckBox, checked, palette, on_toggle).child(mark)
}

/// A full-width clickable row with its label first and its control last.
fn row(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    role: Role,
    on: bool,
    palette: ThemeColors,
    on_toggle: ClickHandler,
) -> Stateful<gpui::Div> {
    let label = label.into();
    h_stack()
        .id(id)
        .role(role)
        .aria_label(label.clone())
        .aria_toggled(toggled(on))
        .tab_index(0)
        .focus_ring(palette)
        .items_center()
        .justify_between()
        .gap(px(metrics::INTERNAL_PAGE_GROUP_GAP))
        .w_full()
        .h(px(metrics::SETTINGS_ROW_HEIGHT))
        .px(px(metrics::MENU_ITEM_HORIZONTAL_PADDING))
        .rounded(px(metrics::CONTROL_RADIUS))
        .text_size(px(metrics::INTERNAL_PAGE_BODY_SIZE))
        .text_color(rgb(palette.text_primary))
        .hover(|style| style.bg(rgba(palette.hover_surface)))
        .owns_clicks()
        .pressable()
        .on_click(on_toggle)
        .child(label)
}
