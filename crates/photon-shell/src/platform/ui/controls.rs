//! Small controls shared by menus and Photon's own pages: segmented choices,
//! colour swatches, switches, checkbox marks and text fields, each showing
//! hover, chosen and focus the same way with the theme's roles. The dropdown
//! is in `dropdown`.

use gpui::{
    App, ElementId, MouseButton, Role, SharedString, Toggled, Window, div, prelude::*, px, rgb,
    rgb_to_hsla, rgba,
};
use gpui_elements::editable_text::EditableTextElement;

use super::icons::check_icon;
use super::layout::{ControlSurface, Elevated, Elevation, h_stack};
use super::motion::{AnimateIn, Edge, Entrance, Speed};
use super::{ClickHandler, metrics, theme::ThemeColors};

/// One option in [`choices`].
pub(super) struct Choice {
    pub(super) label: SharedString,
    pub(super) chosen: bool,
    pub(super) on_choose: ClickHandler,
}

/// Mutually exclusive options in a segmented control, the chosen one filled.
/// Keeps clicks on a control to itself, so they never reach what is behind
/// it: no window move from the titlebar, and no browser menu from empty
/// sidebar space.
pub(super) trait OwnsClicks: InteractiveElement + Sized {
    fn owns_clicks(self) -> Self {
        self.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
    }
}

impl<E: InteractiveElement> OwnsClicks for E {}

/// The ring a control shows while it has keyboard focus: an accent border,
/// transparent at rest so showing it never moves the control's content.
/// Call it before any border the control has at rest, which then shows
/// until it is focused.
pub(super) trait FocusRing: InteractiveElement + Styled + Sized {
    fn focus_ring(self, palette: ThemeColors) -> Self {
        self.focus_ring_in(palette.chosen)
    }

    /// The ring in `color`, for a control whose own colour is the accent.
    fn focus_ring_in(self, color: u32) -> Self {
        self.border_1()
            .border_color(gpui::transparent_black())
            .focus_visible(move |style| style.border_color(rgb(color)))
    }
}

impl<E: InteractiveElement + Styled> FocusRing for E {}

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
            // On the chosen segment's accent, the ring takes its text colour.
            .focus_ring_in(if option.chosen {
                palette.on_chosen
            } else {
                palette.chosen
            })
            .flex_1()
            .items_center()
            .justify_center()
            .h(px(metrics::SEGMENT_HEIGHT))
            .px(px(metrics::SEGMENT_HORIZONTAL_PADDING))
            .rounded(px(metrics::SEGMENT_RADIUS))
            .text_size(px(metrics::BUTTON_FONT_SIZE))
            .owns_clicks()
            .on_click(option.on_choose)
            .child(option.label);
        if option.chosen {
            segment
                .bg(rgb(palette.chosen))
                .text_color(rgb(palette.on_chosen))
                .hover(|style| style.bg(rgb(palette.chosen_hover)))
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
        .control_surface(palette)
        .children(options)
}

/// One colour in [`swatches`].
pub(super) struct Swatch {
    pub(super) label: SharedString,
    /// The colour shown, as an RGB token from the theme.
    pub(super) color: u32,
    pub(super) chosen: bool,
    pub(super) on_choose: ClickHandler,
}

/// Mutually exclusive colours as round swatches, the chosen one ringed.
pub(super) fn swatches(
    id: &'static str,
    label: &'static str,
    options: Vec<Swatch>,
    palette: ThemeColors,
) -> impl IntoElement {
    let options = options.into_iter().enumerate().map(|(index, option)| {
        h_stack()
            .id((id, index))
            .role(Role::RadioButton)
            .aria_label(option.label)
            .aria_toggled(toggled(option.chosen))
            .tab_index(0)
            .size(px(metrics::SWATCH_RING_SIZE))
            .items_center()
            .justify_center()
            .rounded_full()
            .border_2()
            .border_color(if option.chosen {
                rgb_to_hsla(rgb(palette.chosen))
            } else {
                gpui::transparent_black()
            })
            .when(!option.chosen, |ring| {
                ring.hover(|style| style.border_color(rgba(palette.selected_surface)))
            })
            // Focus rings in the text colour, apart from the chosen ring's
            // accent, so a focused swatch never looks chosen.
            .focus_visible(|style| style.border_color(rgb(palette.text_primary)))
            .owns_clicks()
            .on_click(option.on_choose)
            .child(
                div()
                    .size(px(metrics::SWATCH_SIZE))
                    .rounded_full()
                    .border_1()
                    .border_color(rgba(palette.control_border))
                    .bg(rgb(option.color)),
            )
    });
    h_stack()
        .id(id)
        .role(Role::RadioGroup)
        .aria_label(label)
        .flex_wrap()
        .gap(px(metrics::SWATCH_GAP))
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
            .when(on, |track| {
                track
                    .justify_end()
                    .bg(rgb(palette.chosen))
                    .hover(|style| style.bg(rgb(palette.chosen_hover)))
            })
            .when(!on, |track| track.bg(rgba(palette.selected_surface)))
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
            return mark.border_color(rgba(palette.control_border));
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

/// A dialog's text field: its box, with the text centred in it and
/// scrolling sideways when long.
pub(super) fn text_field(input: EditableTextElement, palette: ThemeColors) -> gpui::Div {
    h_stack()
        .items_center()
        .w_full()
        .h(px(metrics::BUTTON_HEIGHT))
        .px(px(metrics::MENU_ITEM_HORIZONTAL_PADDING))
        .control_surface(palette)
        .child(
            themed_text_input(input, palette)
                .flex_1()
                .min_w_0()
                .whitespace_nowrap()
                .overflow_x_scroll(),
        )
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
