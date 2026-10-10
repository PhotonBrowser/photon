//! Settings rows for Photon's own pages: a label with a switch or checkbox,
//! the whole row clickable.

use gpui::{ElementId, MouseButton, Role, SharedString, Stateful, prelude::*, px, rgb, rgba};

use super::super::controls::{check_mark, switch, toggled};
use super::super::layout::h_stack;
use super::super::{ClickHandler, metrics, theme::ThemeColors};

/// A labelled on/off switch.
pub(super) fn switch_row(
    id: &'static str,
    label: &'static str,
    on: bool,
    palette: ThemeColors,
    on_toggle: ClickHandler,
) -> impl IntoElement {
    row(id, label, Role::Switch, on, palette, on_toggle).child(switch(id, on, palette))
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
        .focus_visible(|style| style.border_1().border_color(rgb(palette.chosen)))
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
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(on_toggle)
        .child(label)
}
