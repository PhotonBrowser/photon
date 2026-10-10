//! Small controls shared by menus and Photon's own pages: segmented choices,
//! switches and checkbox marks. Each shows hover, chosen and focus the same
//! way, using the theme's roles.

use gpui::{
    App, ElementId, MouseButton, Role, SharedString, Toggled, Window, div, prelude::*, px, rgb,
    rgb_to_hsla, rgba,
};
use gpui_elements::editable_text::EditableTextElement;

use super::icons::check_icon;
use super::layout::{Elevated, Elevation, h_stack};
use super::motion::{AnimateIn, Edge, Entrance, Speed};
use super::{ClickHandler, metrics, theme::ThemeColors};

/// One option in [`choices`].
pub(super) struct Choice {
    pub(super) label: SharedString,
    pub(super) chosen: bool,
    pub(super) on_choose: ClickHandler,
}

/// Mutually exclusive options in a segmented control, the chosen one filled.
pub(super) fn choices(
    id: &'static str,
    label: &'static str,
    options: Vec<Choice>,
    palette: ThemeColors,
) -> impl IntoElement {
    let options = options.into_iter().enumerate().map(|(index, option)| {
        let segment = h_stack()
            .id((id, index))
            .role(Role::RadioButton)
            .aria_label(option.label.clone())
            .aria_toggled(toggled(option.chosen))
            .tab_index(0)
            .focus_visible(|style| style.border_1().border_color(rgb(palette.chosen)))
            .flex_1()
            .items_center()
            .justify_center()
            .h(px(metrics::SEGMENT_HEIGHT))
            .px(px(metrics::SEGMENT_HORIZONTAL_PADDING))
            .rounded(px(metrics::SEGMENT_RADIUS))
            .text_size(px(metrics::BUTTON_FONT_SIZE))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(option.on_choose)
            .child(option.label);
        if option.chosen {
            segment
                .bg(rgb(palette.chosen))
                .text_color(rgb(palette.on_chosen))
        } else {
            segment
                .text_color(rgb(palette.text_primary))
                .hover(|style| style.bg(rgba(palette.hover_surface)))
        }
    });
    h_stack()
        .id(id)
        .role(Role::RadioGroup)
        .aria_label(label)
        .gap(px(metrics::SEGMENT_GAP))
        .p(px(metrics::SEGMENT_INSET))
        .rounded(px(metrics::CONTROL_RADIUS))
        .bg(rgba(palette.field))
        .children(options)
}

/// An on/off switch whose knob slides across when it changes. Its row owns
/// the click.
pub(super) fn switch(id: &'static str, on: bool, palette: ThemeColors) -> Switch {
    Switch { id, on, palette }
}

#[derive(IntoElement)]
pub(super) struct Switch {
    id: &'static str,
    on: bool,
    palette: ThemeColors,
}

impl RenderOnce for Switch {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self { id, on, palette } = self;
        let travel =
            metrics::SWITCH_WIDTH - metrics::SWITCH_KNOB_SIZE - 2.0 * metrics::SWITCH_INSET;
        let knob = div()
            .size(px(metrics::SWITCH_KNOB_SIZE))
            .rounded_full()
            .bg(rgb(palette.on_chosen))
            .elevated(Elevation::Low);
        let knob = match changes_since_shown(ElementId::from(id), on, window, cx) {
            0 => knob.into_any_element(),
            changes => knob
                .animate_in(
                    (ElementId::from(id), changes.to_string()),
                    Entrance::new()
                        .slide_from(if on { Edge::Left } else { Edge::Right }, travel)
                        .speed(Speed::Quick),
                )
                .into_any_element(),
        };
        h_stack()
            .flex_shrink_0()
            .w(px(metrics::SWITCH_WIDTH))
            .p(px(metrics::SWITCH_INSET))
            .rounded_full()
            .when(on, |track| track.justify_end().bg(rgb(palette.chosen)))
            .when(!on, |track| track.bg(rgba(palette.control_hover_surface)))
            .child(knob)
    }
}

/// A checkbox's box, whose mark fades in when it is checked. Its row owns
/// the click.
pub(super) fn check_mark(id: ElementId, checked: bool, palette: ThemeColors) -> CheckMark {
    CheckMark {
        id,
        checked,
        palette,
    }
}

#[derive(IntoElement)]
pub(super) struct CheckMark {
    id: ElementId,
    checked: bool,
    palette: ThemeColors,
}

impl RenderOnce for CheckMark {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self {
            id,
            checked,
            palette,
        } = self;
        let changes = changes_since_shown(id.clone(), checked, window, cx);
        let mark = h_stack()
            .flex_shrink_0()
            .size(px(metrics::CHECKBOX_SIZE))
            .items_center()
            .justify_center()
            .rounded(px(metrics::CHECKBOX_RADIUS))
            .border_1();
        if !checked {
            return mark.border_color(rgba(palette.menu_border));
        }
        let check = div().child(check_icon(palette.on_chosen, metrics::CHECKBOX_ICON_SIZE));
        mark.border_color(rgb(palette.chosen))
            .bg(rgb(palette.chosen))
            .child(match changes {
                0 => check.into_any_element(),
                changes => check
                    .animate_in((id, changes.to_string()), Entrance::fade())
                    .into_any_element(),
            })
    }
}

/// How many times a control's value has changed while it stayed on screen.
/// Controls animate only after a change, never just for appearing.
fn changes_since_shown(id: ElementId, value: bool, window: &mut Window, cx: &mut App) -> usize {
    struct Memory {
        value: bool,
        changes: usize,
    }
    let memory = window.use_keyed_state(id, cx, |_, _| Memory { value, changes: 0 });
    let (last, changes) = {
        let memory = memory.read(cx);
        (memory.value, memory.changes)
    };
    if last == value {
        return changes;
    }
    // Updated without notifying, so remembering does not redraw the view.
    memory.update(cx, |memory, _| {
        memory.value = value;
        memory.changes += 1;
    });
    changes + 1
}

/// Applies the theme's caret, selection and blink to a text field.
pub(super) fn themed_text_input(
    input: EditableTextElement,
    palette: ThemeColors,
) -> EditableTextElement {
    input
        .caret_color(rgb_to_hsla(rgb(palette.chosen)))
        .selection_color(rgb_to_hsla(rgba(palette.selection)))
        .caret_blink_interval_500ms()
}

pub(super) fn toggled(on: bool) -> Toggled {
    if on { Toggled::True } else { Toggled::False }
}
