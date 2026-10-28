//! Small controls shared by menus and Photon's own pages: segmented choices,
//! switches, checkbox marks and text fields, each showing hover, chosen and
//! focus the same way with the theme's roles. The dropdown is in `dropdown`.

use crate::platform::haptics::Feedback;
use gpui::{
    App, ClickEvent, Context, DispatchPhase, Div, DragMoveEvent, ElementId, FocusHandle,
    InteractiveElement, KeyDownEvent, MouseButton, MouseClickEvent, MouseDownEvent, Render,
    RenderOnce, Role, SharedString, Stateful, StatefulInteractiveElement, Toggled, Window, div,
    prelude::*, px, rgb, rgb_to_hsla, rgba,
};
use gpui_elements::editable_text::EditableTextElement;

use super::icons::check_icon;
use super::layout::{ControlSurface, Elevated, Elevation, h_stack};
use super::motion::SurfaceFade;
use super::motion::{AnimateIn, fade};
use super::motion::{control_thumb_transition, pressable, pressable_with_focus};
use super::{ClickHandler, metrics, theme::ThemeColors};
use std::cell::Cell;
use std::rc::Rc;

/// Runs a switch activation and haptic at most once for each mouse press.
pub(super) fn haptic_switch_toggle(
    on_toggle: Rc<ClickHandler>,
    activated: Rc<Cell<bool>>,
) -> Rc<ClickHandler> {
    Rc::new(Box::new(move |event, window, cx| {
        if matches!(event, ClickEvent::Mouse(_)) && activated.replace(true) {
            return;
        }
        on_toggle(event, window, cx);
        crate::platform::haptics::play(cx, Feedback::StateChange);
    }))
}

#[derive(Clone, Copy)]
struct SwitchDrag {
    id: &'static str,
}

#[derive(Default)]
struct SwitchInteraction {
    down: Option<MouseDownEvent>,
    start_position: f32,
    position: Option<f32>,
    feedback_state: Option<bool>,
    dragging: bool,
}

struct SwitchDragView;

impl Render for SwitchDragView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size(px(1.0)).opacity(0.0)
    }
}

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
pub(super) trait OwnsClicks: Sized {
    fn owns_clicks(self) -> Self {
        self
    }
}

impl OwnsClicks for Div {
    fn owns_clicks(self) -> Self {
        self.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
    }
}

impl<E> OwnsClicks for Stateful<E>
where
    Stateful<E>: StatefulInteractiveElement + Styled,
{
    fn owns_clicks(self) -> Self {
        self.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
    }
}

/// Adds the shared pressed scale to a control that responds to user input.
pub(super) trait Pressable: StatefulInteractiveElement + Styled + Sized {
    fn pressable(self) -> gpui_motion::PressScaleElement<Self>
    where
        Self: IntoElement + 'static,
    {
        pressable(self)
    }

    fn pressable_with_focus(self, focus_handle: FocusHandle) -> gpui_motion::PressScaleElement<Self>
    where
        Self: IntoElement + 'static,
    {
        pressable_with_focus(self, focus_handle)
    }
}

impl<E: StatefulInteractiveElement + Styled> Pressable for E {}

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
        self.border_2()
            .border_color(gpui::transparent_black())
            .focus_visible_style(move |style| style.border_color(rgb(color)))
    }
}

impl<E: InteractiveElement + Styled> FocusRing for E {}

#[derive(Clone, Copy)]
pub(super) enum RovingAxis {
    Horizontal,
    Vertical,
    Any,
}

#[derive(Clone)]
pub(super) struct RovingFocus {
    handles: Rc<Vec<FocusHandle>>,
    actions: Rc<Vec<Rc<ClickHandler>>>,
    selected: usize,
}

impl RovingFocus {
    pub(super) fn new(
        id: &'static str,
        selected: usize,
        actions: Vec<Rc<ClickHandler>>,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let keys = (0..actions.len()).collect();
        Self::keyed(id, selected, keys, actions, window, cx)
    }

    /// Like `new`, but each item's focus is keyed by its stable `keys`
    /// entry rather than its position, so focus follows an item when the
    /// items are reordered or one is removed.
    pub(super) fn keyed<K: std::fmt::Display>(
        id: &'static str,
        selected: usize,
        keys: Vec<K>,
        actions: Vec<Rc<ClickHandler>>,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        debug_assert_eq!(keys.len(), actions.len());
        let selected = selected.min(actions.len().saturating_sub(1));
        let actions = Rc::new(actions);
        let handles = keys
            .iter()
            .map(|key| {
                window
                    .use_keyed_state(
                        ElementId::Name(format!("{id}-focus-{key}").into()),
                        cx,
                        |_, cx| cx.focus_handle(),
                    )
                    .read(cx)
                    .clone()
            })
            .collect();
        Self {
            handles: Rc::new(handles),
            actions,
            selected,
        }
    }

    pub(super) fn selected(&self, index: usize) -> bool {
        index == self.selected
    }

    pub(super) fn handle(&self, index: usize) -> FocusHandle {
        self.handles[index].clone()
    }

    pub(super) fn on_click(
        &self,
        index: usize,
    ) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
        let focus = self.clone();
        move |event, window, cx| {
            (focus.actions[index])(event, window, cx);
            window.focus(&focus.handles[index], cx);
        }
    }

    pub(super) fn on_key_down(
        &self,
        index: usize,
        axis: RovingAxis,
    ) -> impl Fn(&KeyDownEvent, &mut Window, &mut App) + 'static {
        let focus = self.clone();
        move |event, window, cx| {
            if event.keystroke.modifiers.modified() || focus.handles.is_empty() {
                return;
            }
            let offset = roving_axis_offset(event.keystroke.key.as_str(), axis);
            let Some(offset) = offset else { return };
            let next = (index as isize + offset).rem_euclid(focus.handles.len() as isize) as usize;
            (focus.actions[next])(&ClickEvent::default(), window, cx);
            window.focus(&focus.handles[next], cx);
            window.prevent_default();
            cx.stop_propagation();
        }
    }
}

fn roving_axis_offset(key: &str, axis: RovingAxis) -> Option<isize> {
    match (axis, key) {
        (RovingAxis::Horizontal, "left") | (RovingAxis::Vertical, "up") => Some(-1),
        (RovingAxis::Horizontal, "right") | (RovingAxis::Vertical, "down") => Some(1),
        (RovingAxis::Any, "left" | "up") => Some(-1),
        (RovingAxis::Any, "right" | "down") => Some(1),
        _ => None,
    }
}

pub(super) fn choices(
    id: &'static str,
    label: &'static str,
    options: Vec<Choice>,
    palette: ThemeColors,
) -> impl IntoElement {
    ChoiceGroup {
        id,
        label,
        options,
        palette,
    }
}

#[derive(IntoElement)]
struct ChoiceGroup {
    id: &'static str,
    label: &'static str,
    options: Vec<Choice>,
    palette: ThemeColors,
}

impl RenderOnce for ChoiceGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self {
            id,
            label,
            options,
            palette,
        } = self;
        let selected = options.iter().position(|option| option.chosen).unwrap_or(0);
        let options = options
            .into_iter()
            .map(|option| (option.label, option.chosen, Rc::new(option.on_choose)))
            .collect::<Vec<_>>();
        let focus = RovingFocus::new(
            id,
            selected,
            options
                .iter()
                .map(|(_, _, action)| action.clone())
                .collect(),
            window,
            cx,
        );
        let options = options
            .into_iter()
            .enumerate()
            .map(|(index, (label, chosen, _))| {
                let segment = h_stack()
                    .id((id, index))
                    .role(Role::RadioButton)
                    .aria_label(label.clone())
                    .aria_toggled(toggled(chosen))
                    .track_focus(&focus.handle(index))
                    .tab_index(index as isize)
                    .tab_stop(focus.selected(index))
                    // On the chosen segment's accent, the ring takes its text colour.
                    .focus_ring_in(if chosen {
                        palette.on_chosen
                    } else {
                        palette.chosen
                    })
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .h(px(metrics::SEGMENT_HEIGHT))
                    .px(px(metrics::SEGMENT_HORIZONTAL_PADDING))
                    .rounded(px(metrics::segment_radius()))
                    .text_size(px(metrics::BUTTON_FONT_SIZE))
                    .owns_clicks()
                    .pressable()
                    .on_click(focus.on_click(index))
                    .on_key_down(focus.on_key_down(index, RovingAxis::Horizontal))
                    .child(label);
                if chosen {
                    segment
                        .bg(rgb(palette.chosen))
                        .text_color(rgb(palette.on_chosen))
                        .surface_fade()
                        .hover_style(|style| style.bg(rgb(palette.chosen_hover)))
                } else {
                    segment
                        .text_color(rgb(palette.text_primary))
                        .surface_fade()
                        .hover_style(|style| style.bg(rgba(palette.hover_surface)))
                }
            });
        h_stack()
            .id(id)
            .role(Role::RadioGroup)
            .aria_label(label)
            .tab_group()
            .gap(px(metrics::SEGMENT_GAP))
            .p(px(metrics::SEGMENT_INSET))
            .control_surface(palette)
            .rounded(px(metrics::choices_radius()))
            .children(options)
    }
}

#[cfg(test)]
mod roving_focus_tests {
    use super::{RovingAxis, roving_axis_offset};

    #[test]
    fn roving_focus_follows_group_orientation() {
        assert_eq!(roving_axis_offset("left", RovingAxis::Horizontal), Some(-1));
        assert_eq!(roving_axis_offset("down", RovingAxis::Horizontal), None);
        assert_eq!(roving_axis_offset("up", RovingAxis::Vertical), Some(-1));
        assert_eq!(roving_axis_offset("right", RovingAxis::Vertical), None);
        assert_eq!(roving_axis_offset("up", RovingAxis::Any), Some(-1));
        assert_eq!(roving_axis_offset("right", RovingAxis::Any), Some(1));
    }
}

pub(super) fn switch(
    id: &'static str,
    on: bool,
    palette: ThemeColors,
    label: Option<&'static str>,
    on_toggle: Rc<ClickHandler>,
    on_activate: Rc<ClickHandler>,
    dragged: Rc<Cell<bool>>,
) -> Switch {
    Switch {
        id,
        on,
        palette,
        label,
        on_toggle,
        on_activate,
        dragged,
        disabled: false,
    }
}

#[derive(IntoElement)]
pub(super) struct Switch {
    id: &'static str,
    on: bool,
    palette: ThemeColors,
    label: Option<&'static str>,
    on_toggle: Rc<ClickHandler>,
    on_activate: Rc<ClickHandler>,
    dragged: Rc<Cell<bool>>,
    disabled: bool,
}

impl Switch {
    /// Shows the switch dimmed, ignoring the pointer, for something that
    /// cannot be changed.
    pub(super) fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for Switch {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self {
            id,
            on,
            palette,
            label,
            on_toggle,
            on_activate,
            dragged,
            disabled,
        } = self;
        let animations = super::settings::Settings::motion(cx);
        let interaction = window.use_keyed_state(
            ElementId::Name(format!("{id}-switch-interaction").into()),
            cx,
            |_, _| SwitchInteraction::default(),
        );
        let (is_pressed, is_dragging, drag_position) = {
            let interaction = interaction.read(cx);
            (
                interaction.down.is_some(),
                interaction.dragging,
                interaction.position,
            )
        };
        let travel =
            metrics::SWITCH_WIDTH - metrics::SWITCH_KNOB_WIDTH - 2.0 * metrics::SWITCH_INSET;
        let track_inner_width = metrics::SWITCH_WIDTH - 2.0 * metrics::SWITCH_INSET;
        let position = drag_position.unwrap_or(if on { travel } else { 0.0 });
        let progress = (position / travel).clamp(0.0, 1.0);
        let visual_on = progress >= 0.5;
        let thumb_width = metrics::SWITCH_KNOB_WIDTH
            + if is_pressed {
                metrics::SWITCH_KNOB_HELD_STRETCH
            } else {
                0.0
            };
        // Keep the held expansion centred where possible, then clamp it so
        // the capsule only stretches toward the inside edge of the track.
        let thumb_left = (position - (thumb_width - metrics::SWITCH_KNOB_WIDTH) / 2.0)
            .clamp(0.0, track_inner_width - thumb_width);

        let thumb = div()
            .absolute()
            .top(px(0.0))
            .h(px(metrics::SWITCH_KNOB_HEIGHT))
            .rounded_full()
            .bg(rgb(palette.on_chosen))
            .elevated(Elevation::Low);
        let thumb = control_thumb_transition(
            ElementId::Name(format!("{id}-switch-thumb-motion").into()),
            thumb,
            thumb_left,
            thumb_width,
            is_dragging,
            &animations,
        );

        let track = div()
            .id(ElementId::Name(format!("{id}-switch-track").into()))
            .relative()
            .flex_shrink_0()
            .w(px(metrics::SWITCH_WIDTH))
            .h(px(metrics::SWITCH_KNOB_HEIGHT + 2.0 * metrics::SWITCH_INSET))
            .rounded_full()
            .when(visual_on, |track| {
                track
                    .bg(rgb(palette.chosen))
                    .surface_fade()
                    .when(!disabled, |track| {
                        track.hover_style(|style| style.bg(rgb(palette.chosen_hover)))
                    })
            })
            .when(!visual_on, |track| {
                track
                    .bg(rgba(palette.selected_surface))
                    .surface_fade()
                    .when(!disabled, |track| {
                        track.hover_style(|style| style.bg(rgba(palette.hover_surface)))
                    })
            })
            .child(
                div()
                    .id(ElementId::Name(format!("{id}-switch-inner").into()))
                    .absolute()
                    .left(px(metrics::SWITCH_INSET))
                    .top(px(metrics::SWITCH_INSET))
                    .w(px(track_inner_width))
                    .h(px(metrics::SWITCH_KNOB_HEIGHT))
                    .overflow_x_hidden()
                    .child(thumb),
            )
            .into_any_element();

        let mut control = h_stack()
            .id(ElementId::Name(format!("{id}-switch-control").into()))
            .items_center()
            .flex_shrink_0()
            .rounded_full();
        if let Some(label) = label {
            control = control
                .role(Role::Switch)
                .aria_label(label)
                .aria_toggled(toggled(on))
                .tab_index(0)
                .focus_ring(palette);
        }
        if disabled {
            let opacity = crate::platform::theme_config::theme()
                .opacity
                .disabled_control;
            return control
                .aria_disabled(true)
                .tab_index(-1)
                .opacity(opacity)
                .child(track);
        }
        let interaction_on_down = interaction.clone();
        let interaction_on_drag = interaction.clone();
        let interaction_on_move = interaction.clone();
        let interaction_on_up = interaction;
        let drag_marker_start = dragged.clone();
        let click_dragged = dragged;
        let activation_toggle = on_activate;
        let drag_toggle = on_toggle;
        control
            .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                interaction_on_down.update(cx, |interaction, cx| {
                    *interaction = SwitchInteraction {
                        down: Some(event.clone()),
                        start_position: if on { travel } else { 0.0 },
                        feedback_state: Some(on),
                        ..SwitchInteraction::default()
                    };
                    cx.notify();
                });
            })
            .on_mouse_up_all(move |event, phase, _hitbox, window, cx| {
                if event.button != MouseButton::Left || phase != DispatchPhase::Capture {
                    return;
                }

                let drag = interaction_on_up.update(cx, |interaction, cx| {
                    let drag = interaction
                        .down
                        .clone()
                        .filter(|_| interaction.dragging)
                        .map(|down| (down, interaction.start_position, interaction.feedback_state));
                    if interaction.down.is_some() || interaction.dragging {
                        *interaction = SwitchInteraction::default();
                        cx.notify();
                    }
                    drag
                });

                if let Some((down, start_position, feedback_state)) = drag {
                    let position = (start_position + f32::from(event.position.x)
                        - f32::from(down.position.x))
                    .clamp(0.0, travel);
                    let next_on = position >= travel / 2.0;
                    if feedback_state.unwrap_or(on) != next_on {
                        crate::platform::haptics::play(cx, Feedback::StateChange);
                    }
                    if next_on != on {
                        drag_toggle(
                            &ClickEvent::Mouse(MouseClickEvent {
                                down,
                                up: event.clone(),
                            }),
                            window,
                            cx,
                        );
                    }
                }
            })
            .on_drag(SwitchDrag { id }, move |_, _, _, cx| {
                drag_marker_start.set(true);
                interaction_on_drag.update(cx, |interaction, cx| {
                    if !interaction.dragging {
                        interaction.dragging = true;
                        cx.notify();
                    }
                });
                cx.new(|_| SwitchDragView)
            })
            .on_drag_move(move |event: &DragMoveEvent<SwitchDrag>, _, cx| {
                if event.drag(cx).id != id {
                    return;
                }

                let (start_x, start_position, feedback_state) = {
                    let interaction = interaction_on_move.read(cx);
                    (
                        interaction
                            .down
                            .as_ref()
                            .map(|down| f32::from(down.position.x)),
                        interaction.start_position,
                        interaction.feedback_state,
                    )
                };
                let Some(start_x) = start_x else {
                    return;
                };
                let position = (start_position + f32::from(event.event.position.x) - start_x)
                    .clamp(0.0, travel);
                let next_on = position >= travel / 2.0;
                let feedback_changed = feedback_state.unwrap_or(on) != next_on;
                interaction_on_move.update(cx, |interaction, cx| {
                    if interaction.position != Some(position)
                        || interaction.feedback_state != Some(next_on)
                    {
                        interaction.position = Some(position);
                        interaction.feedback_state = Some(next_on);
                        cx.notify();
                    }
                });
                if feedback_changed {
                    crate::platform::haptics::play(cx, Feedback::StateChange);
                }
            })
            .on_click(move |event, window, cx| {
                if !click_dragged.replace(false) {
                    activation_toggle(event, window, cx);
                }
                cx.stop_propagation();
            })
            .child(track)
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
            .rounded(px(metrics::checkbox_radius()))
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
                    .animate_in(
                        (id, changes.to_string()),
                        fade(),
                        &super::settings::Settings::motion(cx),
                    )
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
        .text_color(rgb(palette.text_primary))
        .caret_color(rgb_to_hsla(rgb(palette.text_primary)))
        .selection_color(rgb_to_hsla(rgba(palette.selection)))
        .caret_blink_interval_500ms()
}

pub(super) fn toggled(on: bool) -> Toggled {
    if on { Toggled::True } else { Toggled::False }
}
