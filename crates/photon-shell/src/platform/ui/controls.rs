//! Small controls shared by menus and Photon's own pages: segmented choices,
//! colour swatches, switches, checkbox marks and text fields, each showing
//! hover, chosen and focus the same way with the theme's roles. The dropdown
//! is in `dropdown`.

use gpui::{
    AccessibleAction, App, Bounds, ClickEvent, Context, DispatchPhase, Div, DragMoveEvent,
    ElementId, FontWeight, InteractiveElement, KeyDownEvent, MouseButton, MouseClickEvent,
    MouseDownEvent, Pixels, Render, Role, SharedString, Stateful, StatefulInteractiveElement,
    Toggled, Window, div, prelude::*, px, rgb, rgb_to_hsla, rgba,
};
use gpui_elements::editable_text::EditableTextElement;

use super::icons::check_icon;
use super::layout::v_stack;
use super::layout::{ControlSurface, Elevated, Elevation, h_stack};
use super::motion::{AnimateIn, fade};
use super::motion::{
    CONTROL_THUMB_HELD_STRETCH_X, SLIDER_EDGE_STRETCH_MAX, SLIDER_EDGE_STRETCH_SPEED_RANGE,
    SLIDER_EDGE_STRETCH_SPEED_THRESHOLD, control_scale_transition, control_thumb_transition,
    control_thumb_transition_tracking, pressable,
};
use super::{ClickHandler, metrics, theme::ThemeColors};
use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;

struct IntensitySliderDrag;

#[derive(Clone, Copy)]
struct SliderDragSample {
    position: Pixels,
    time: Instant,
}

#[derive(Clone, Copy, Default, PartialEq)]
struct SliderStretch {
    scale: f32,
    offset: f32,
}

struct SliderDragFeedback {
    direction: Option<f32>,
    stretch: SliderStretch,
}

impl Render for IntensitySliderDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size(px(1.0)).opacity(0.0)
    }
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
    dragging: bool,
}

struct SwitchDragView;

impl Render for SwitchDragView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size(px(1.0)).opacity(0.0)
    }
}

#[derive(IntoElement)]
struct StretchThumb {
    id: ElementId,
    width: f32,
    height: f32,
    extension: f32,
    direction: f32,
    color: u32,
}

impl RenderOnce for StretchThumb {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self {
            id,
            width,
            height,
            extension,
            direction,
            color,
        } = self;
        let offset = if direction < 0.0 {
            -extension
        } else if direction > 0.0 {
            0.0
        } else {
            -extension * 0.5
        };

        let animations = super::settings::Settings::motion(cx);

        div()
            .relative()
            .w(px(width))
            .h(px(height))
            .flex_shrink_0()
            .child(control_thumb_transition(
                div()
                    .id(id)
                    .absolute()
                    .top(px(0.0))
                    .left(px(offset))
                    .w(px(width + extension))
                    .h(px(height))
                    .rounded_full()
                    .bg(rgb(color))
                    .elevated(Elevation::Low),
                &animations,
            ))
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
    fn pressable(self) -> Self {
        pressable(self)
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
            .pressable()
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

pub(super) fn intensity_slider(
    id: &'static str,
    label: &'static str,
    value: u8,
    max_value: u8,
    value_step: u8,
    palette: ThemeColors,
    on_change: impl Fn(u8, &mut App) + 'static,
) -> IntensitySlider {
    IntensitySlider {
        id,
        label,
        value,
        max_value,
        value_step,
        palette,
        on_change: Rc::new(on_change),
    }
}

type IntensityChange = Rc<dyn Fn(u8, &mut App)>;

#[derive(IntoElement)]
pub(super) struct IntensitySlider {
    id: &'static str,
    label: &'static str,
    value: u8,
    max_value: u8,
    value_step: u8,
    palette: ThemeColors,
    on_change: IntensityChange,
}

impl RenderOnce for IntensitySlider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self {
            id,
            label,
            value,
            max_value,
            value_step,
            palette,
            on_change,
        } = self;
        let max_value = max_value.max(1);
        let value = value.min(max_value);
        let fraction = f32::from(value) / f32::from(max_value);
        let thumb_press = window.use_keyed_state(
            ElementId::Name(format!("{id}-slider-thumb-press").into()),
            cx,
            |_, _| false,
        );
        let drag_sample = window.use_keyed_state(
            ElementId::Name(format!("{id}-slider-drag-sample").into()),
            cx,
            |_, _| None::<SliderDragSample>,
        );
        let elastic_stretch = window.use_keyed_state(
            ElementId::Name(format!("{id}-slider-elastic-stretch").into()),
            cx,
            |_, _| SliderStretch::default(),
        );
        let thumb_direction = window.use_keyed_state(
            ElementId::Name(format!("{id}-slider-thumb-direction").into()),
            cx,
            |_, _| 0.0,
        );
        let thumb_is_pressed = *thumb_press.read(cx);
        let stretch = *elastic_stretch.read(cx);
        let direction = *thumb_direction.read(cx);
        let animations_enabled = super::settings::Settings::get(cx).animations.enabled;
        let animations = super::settings::Settings::motion(cx);
        let press_start = thumb_press.clone();
        let press_end = thumb_press;
        let drag_sample_start = drag_sample.clone();
        let drag_sample_move = drag_sample.clone();
        let drag_sample_end = drag_sample;
        let stretch_start = elastic_stretch.clone();
        let stretch_move = elastic_stretch.clone();
        let stretch_end = elastic_stretch;
        let thumb_direction_start = thumb_direction.clone();
        let thumb_direction_move = thumb_direction.clone();
        let thumb_direction_end = thumb_direction;
        let click_change = on_change.clone();
        let drag_change = on_change.clone();
        let key_change = on_change.clone();
        let increment_change = on_change.clone();
        let decrement_change = on_change.clone();
        let thumb_extension = if thumb_is_pressed {
            metrics::INTENSITY_SLIDER_THUMB_WIDTH * (CONTROL_THUMB_HELD_STRETCH_X - 1.0)
        } else {
            0.0
        };
        let thumb = StretchThumb {
            id: ElementId::Name(format!("{id}-slider-thumb").into()),
            width: metrics::INTENSITY_SLIDER_THUMB_WIDTH,
            height: metrics::INTENSITY_SLIDER_THUMB_HEIGHT,
            extension: thumb_extension,
            direction,
            color: palette.chosen,
        };
        let active_track = div()
            .h(px(metrics::INTENSITY_SLIDER_TRACK_HEIGHT))
            .flex_grow(fraction)
            .rounded_full()
            .bg(rgb(palette.chosen));
        let remaining_track = div()
            .h(px(metrics::INTENSITY_SLIDER_TRACK_HEIGHT))
            .flex_grow(1.0 - fraction)
            .rounded_full()
            .bg(rgba(palette.control_border));
        let track_contents = control_scale_transition(
            div()
                .id(ElementId::Name(format!("{id}-slider-contents").into()))
                .relative()
                .left(px(stretch.offset))
                .flex()
                .flex_row()
                .items_center()
                .w_full()
                .h(px(metrics::INTENSITY_SLIDER_THUMB_HEIGHT))
                .scale_xy(1.0 + stretch.scale, 1.0)
                .child(active_track)
                .child(thumb)
                .child(remaining_track),
            thumb_is_pressed,
            &animations,
        );
        let track = div()
            .id(id)
            .role(Role::Slider)
            .aria_label(label)
            .aria_value(format!("{value}%"))
            .aria_numeric_value(f64::from(value))
            .aria_min_numeric_value(0.0)
            .aria_max_numeric_value(f64::from(max_value))
            .aria_numeric_value_step(f64::from(value_step.max(1)))
            .tab_index(0)
            .focus_ring(palette)
            .cursor_col_resize()
            .flex_1()
            .h(px(metrics::INTENSITY_SLIDER_HEIGHT))
            .px(px(metrics::INTENSITY_SLIDER_THUMB_WIDTH / 2.0))
            .on_mouse_down_all(move |event, phase, hitbox, window, cx| {
                if phase == DispatchPhase::Bubble
                    && event.button == MouseButton::Left
                    && hitbox.is_hovered(window)
                {
                    press_start.update(cx, |pressed, cx| {
                        if !*pressed {
                            *pressed = true;
                            cx.notify();
                        }
                    });
                    drag_sample_start.update(cx, |sample, cx| {
                        *sample = Some(SliderDragSample {
                            position: event.position.x,
                            time: Instant::now(),
                        });
                        cx.notify();
                    });
                    stretch_start.update(cx, |stretch, cx| {
                        if *stretch != SliderStretch::default() {
                            *stretch = SliderStretch::default();
                            cx.notify();
                        }
                    });
                    thumb_direction_start.update(cx, |direction, cx| {
                        if *direction != 0.0 {
                            *direction = 0.0;
                            cx.notify();
                        }
                    });
                    click_change(
                        slider_value_at(event.position.x, hitbox.bounds, max_value),
                        cx,
                    );
                }
            })
            .on_mouse_up_all(move |event, _, _, _, cx| {
                if event.button == MouseButton::Left {
                    press_end.update(cx, |pressed, cx| {
                        if *pressed {
                            *pressed = false;
                            cx.notify();
                        }
                    });
                    drag_sample_end.update(cx, |sample, cx| {
                        if sample.is_some() {
                            *sample = None;
                            cx.notify();
                        }
                    });
                    stretch_end.update(cx, |stretch, cx| {
                        if *stretch != SliderStretch::default() {
                            *stretch = SliderStretch::default();
                            cx.notify();
                        }
                    });
                    thumb_direction_end.update(cx, |direction, cx| {
                        if *direction != 0.0 {
                            *direction = 0.0;
                            cx.notify();
                        }
                    });
                }
            })
            .on_drag(IntensitySliderDrag, |_, _, _, cx| {
                cx.new(|_| IntensitySliderDrag)
            })
            .on_drag_move(move |event: &DragMoveEvent<IntensitySliderDrag>, _, cx| {
                let now = Instant::now();
                let position = event.event.position.x;
                let feedback = slider_drag_feedback(
                    position,
                    event.bounds,
                    *drag_sample_move.read(cx),
                    now,
                    animations_enabled,
                );
                if let Some(next_direction) = feedback.direction {
                    thumb_direction_move.update(cx, |direction, cx| {
                        if *direction != next_direction {
                            *direction = next_direction;
                            cx.notify();
                        }
                    });
                }
                drag_sample_move.update(cx, |sample, cx| {
                    *sample = Some(SliderDragSample {
                        position,
                        time: now,
                    });
                    cx.notify();
                });
                stretch_move.update(cx, |current, cx| {
                    if *current != feedback.stretch {
                        *current = feedback.stretch;
                        cx.notify();
                    }
                });
                drag_change(slider_value_at(position, event.bounds, max_value), cx);
            })
            .on_key_down(move |event: &KeyDownEvent, _, cx| {
                if event.keystroke.modifiers.modified() {
                    return;
                }
                let next = match event.keystroke.key.as_str() {
                    "left" | "down" => Some(value.saturating_sub(value_step.max(1))),
                    "right" | "up" => Some(value.saturating_add(value_step.max(1)).min(max_value)),
                    "home" => Some(0),
                    "end" => Some(max_value),
                    _ => None,
                };
                if let Some(next) = next {
                    cx.stop_propagation();
                    key_change(next, cx);
                }
            })
            .on_a11y_action(AccessibleAction::Increment, move |_, _, cx| {
                increment_change(value.saturating_add(value_step.max(1)).min(max_value), cx);
            })
            .on_a11y_action(AccessibleAction::Decrement, move |_, _, cx| {
                decrement_change(value.saturating_sub(value_step.max(1)), cx);
            })
            .child(track_contents);

        v_stack().gap(px(metrics::FORM_FIELD_GAP)).child(
            gpui::div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(metrics::MENU_ITEM_GAP))
                .child(track)
                .child(
                    gpui::div()
                        .w(px(metrics::INTENSITY_SLIDER_VALUE_WIDTH))
                        .text_size(px(metrics::INTERNAL_PAGE_BODY_SIZE))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(palette.text_secondary))
                        .child(format!("{value}%")),
                ),
        )
    }
}

fn slider_value_at(position: Pixels, bounds: Bounds<Pixels>, max_value: u8) -> u8 {
    let width = f32::from(bounds.size.width) - metrics::INTENSITY_SLIDER_THUMB_WIDTH;
    let offset = f32::from(position)
        - f32::from(bounds.origin.x)
        - metrics::INTENSITY_SLIDER_THUMB_WIDTH / 2.0;
    (offset / width.max(1.0) * f32::from(max_value))
        .round()
        .clamp(0.0, f32::from(max_value)) as u8
}

fn slider_drag_feedback(
    position: Pixels,
    bounds: Bounds<Pixels>,
    sample: Option<SliderDragSample>,
    now: Instant,
    animations_enabled: bool,
) -> SliderDragFeedback {
    let Some(sample) = sample else {
        return SliderDragFeedback {
            direction: None,
            stretch: SliderStretch::default(),
        };
    };

    let delta = f32::from(position - sample.position);
    let direction = (delta != 0.0).then_some(delta.signum());
    let elapsed = now.duration_since(sample.time).as_secs_f32().max(0.001);
    let speed = delta.abs() / elapsed;
    let min_center = bounds.origin.x + px(metrics::INTENSITY_SLIDER_THUMB_WIDTH / 2.0);
    let max_center =
        bounds.origin.x + bounds.size.width - px(metrics::INTENSITY_SLIDER_THUMB_WIDTH / 2.0);
    let edge_zone = metrics::INTENSITY_SLIDER_THUMB_WIDTH;
    let (edge_direction, edge_proximity) = if delta < 0.0 && position < min_center + px(edge_zone) {
        (
            -1.0,
            (f32::from(min_center + px(edge_zone) - position) / edge_zone).clamp(0.0, 1.0),
        )
    } else if delta > 0.0 && position > max_center - px(edge_zone) {
        (
            1.0,
            (f32::from(position - (max_center - px(edge_zone))) / edge_zone).clamp(0.0, 1.0),
        )
    } else {
        (0.0, 0.0)
    };
    let speed_factor = ((speed - SLIDER_EDGE_STRETCH_SPEED_THRESHOLD)
        / SLIDER_EDGE_STRETCH_SPEED_RANGE)
        .clamp(0.0, 1.0);
    let extension = if animations_enabled {
        metrics::INTENSITY_SLIDER_THUMB_WIDTH
            * SLIDER_EDGE_STRETCH_MAX
            * speed_factor
            * edge_proximity
    } else {
        0.0
    };
    let content_width =
        (f32::from(bounds.size.width) - metrics::INTENSITY_SLIDER_THUMB_WIDTH).max(1.0);

    SliderDragFeedback {
        direction,
        stretch: SliderStretch {
            scale: extension / content_width,
            offset: edge_direction * extension * 0.5,
        },
    }
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
            .pressable()
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

/// An on/off switch whose knob slides to the selected side and follows drags.
pub(super) fn switch(
    id: &'static str,
    on: bool,
    palette: ThemeColors,
    label: Option<&'static str>,
    on_toggle: Rc<ClickHandler>,
    dragged: Rc<Cell<bool>>,
) -> Switch {
    Switch {
        id,
        on,
        palette,
        label,
        on_toggle,
        dragged,
    }
}

#[derive(IntoElement)]
pub(super) struct Switch {
    id: &'static str,
    on: bool,
    palette: ThemeColors,
    label: Option<&'static str>,
    on_toggle: Rc<ClickHandler>,
    dragged: Rc<Cell<bool>>,
}

impl RenderOnce for Switch {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self {
            id,
            on,
            palette,
            label,
            on_toggle,
            dragged,
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
            .id(ElementId::Name(format!("{id}-switch-thumb").into()))
            .absolute()
            .left(px(thumb_left))
            .top(px(0.0))
            .w(px(thumb_width))
            .h(px(metrics::SWITCH_KNOB_HEIGHT))
            .rounded_full()
            .bg(rgb(palette.on_chosen))
            .elevated(Elevation::Low);
        let thumb = control_thumb_transition_tracking(thumb, is_dragging, &animations);

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
                    .hover(|style| style.bg(rgb(palette.chosen_hover)))
            })
            .when(!visual_on, |track| {
                track
                    .bg(rgba(palette.selected_surface))
                    .hover(|style| style.bg(rgba(palette.hover_surface)))
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
        let interaction_on_down = interaction.clone();
        let interaction_on_drag = interaction.clone();
        let interaction_on_move = interaction.clone();
        let interaction_on_up = interaction;
        let drag_marker_start = dragged.clone();
        let click_toggle = on_toggle.clone();
        let click_dragged = dragged.clone();
        let drag_toggle = on_toggle;
        control
            .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                interaction_on_down.update(cx, |interaction, cx| {
                    *interaction = SwitchInteraction {
                        down: Some(event.clone()),
                        start_position: if on { travel } else { 0.0 },
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
                        .map(|down| (down, interaction.start_position));
                    if interaction.down.is_some() || interaction.dragging {
                        *interaction = SwitchInteraction::default();
                        cx.notify();
                    }
                    drag
                });

                if let Some((down, start_position)) = drag {
                    let position = (start_position + f32::from(event.position.x)
                        - f32::from(down.position.x))
                    .clamp(0.0, travel);
                    if (position >= travel / 2.0) != on {
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

                let (start_x, start_position) = {
                    let interaction = interaction_on_move.read(cx);
                    (
                        interaction
                            .down
                            .as_ref()
                            .map(|down| f32::from(down.position.x)),
                        interaction.start_position,
                    )
                };
                let Some(start_x) = start_x else {
                    return;
                };
                let position = (start_position + f32::from(event.event.position.x) - start_x)
                    .clamp(0.0, travel);
                interaction_on_move.update(cx, |interaction, cx| {
                    if interaction.position != Some(position) {
                        interaction.position = Some(position);
                        cx.notify();
                    }
                });
            })
            .on_click(move |event, window, cx| {
                if !click_dragged.get() {
                    click_toggle(event, window, cx);
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
        .caret_color(rgb_to_hsla(rgb(palette.chosen)))
        .selection_color(rgb_to_hsla(rgba(palette.selection)))
        .caret_blink_interval_500ms()
}

pub(super) fn toggled(on: bool) -> Toggled {
    if on { Toggled::True } else { Toggled::False }
}
