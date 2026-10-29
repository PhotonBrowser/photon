//! Native theme field, wave intensity slider and detented grain dial.

mod handles;
mod harmony;
mod paint;
mod presets;
use handles::{ColorHandle, PaletteDrag, field_coordinates};
use presets::PresetPage;
#[cfg(test)]
mod tests;

use paint::{
    field_position, noise_at, paint_field, paint_knob, paint_wave, tint_at, tokens, wave_detent,
};

use gpui::{
    AccessibleAction, App, Bounds, ColorExt, Context, DispatchPhase, ElementId, Entity,
    MouseButton, Pixels, Point, Render, Role, Subscription, Window, canvas, div, prelude::*, px,
    rgb, svg,
};
use photon_core::{SpaceColors, SpaceId, SpaceTint, ThemeMode, WindowColor};
use std::{rc::Rc, time::Instant};

use super::{
    appearance_preview::AppearancePreview,
    controls::{FocusRing, OwnsClicks, Pressable, RovingAxis, RovingFocus, toggled},
    layout::{Elevated, Elevation, h_stack, v_stack},
    metrics,
    motion::{Speed, Tween},
    settings::Settings,
    theme::{self, ThemeColors},
    tooltip::label_tooltip,
};
use crate::platform::haptics::{self, Feedback};

fn picker_control(ink: u32) -> gpui::Div {
    div()
        .size(px(metrics::PICKER_MODE_SIZE))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(metrics::control_radius()))
        .hover_style(|style| style.bg(rgb(ink).opacity(tokens().control_hover)))
}

pub(super) type TintChange = Rc<dyn Fn(WindowColor, SpaceColors, Option<u8>, Option<u8>, &mut App)>;

#[derive(Clone, Copy, Default)]
pub(super) struct PickerAppearance {
    pub transparency: Option<u8>,
    pub noise: Option<u8>,
}

pub(super) fn change_space_theme(space_id: SpaceId) -> TintChange {
    Rc::new(move |color, colors, transparency, noise, cx| {
        Settings::update(cx, |settings| {
            if let Some(space) = settings.spaces.get_mut(space_id) {
                space.color = color;
                space.set_colors(colors);
            }
            if let Some(transparency) = transparency {
                if colors.colors().is_empty()
                    || transparency == AppearancePreview::subtle_transparency()
                {
                    settings.transparency = photon_core::Transparency::Subtle;
                    settings.transparency_percent = None;
                } else {
                    settings.transparency_percent = Some(transparency);
                }
            }
            if let Some(noise) = noise {
                settings.background_noise_intensity = noise;
            }
        })
    })
}

#[derive(Clone, Copy, PartialEq)]
enum Control {
    Field,
    Transparency,
    Noise,
}
/// Retains pointer feedback and publishes appearance once per display frame.
#[derive(IntoElement)]
pub(super) struct ThemePicker {
    id: ElementId,
    color: WindowColor,
    colors: SpaceColors,
    space: Option<photon_core::SpaceId>,
    appearance: PickerAppearance,
    on_change: TintChange,
    floating: bool,
    over_glass: bool,
}

pub(super) fn theme_picker(
    id: impl Into<ElementId>,
    color: WindowColor,
    colors: SpaceColors,
    space: Option<photon_core::SpaceId>,
    on_change: TintChange,
) -> ThemePicker {
    ThemePicker {
        id: id.into(),
        color,
        colors,
        space,
        appearance: PickerAppearance::default(),
        on_change,
        floating: false,
        over_glass: false,
    }
}

impl ThemePicker {
    pub(super) fn with_staged_appearance(mut self, appearance: PickerAppearance) -> Self {
        self.appearance = appearance;
        self
    }

    pub(super) fn into_floating(mut self) -> Self {
        self.floating = true;
        self
    }

    /// Leaves the panel's surface to native glass the caller draws beneath it.
    pub(super) fn over_glass(mut self, over_glass: bool) -> Self {
        self.over_glass = over_glass;
        self
    }
}

impl RenderOnce for ThemePicker {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Only the picker entity repaints for local drag/animation frames.
        let state = window.with_global_id(self.id, |id, window| {
            window.with_element_state(id, |state: Option<Entity<PickerState>>, _| {
                let state = state.unwrap_or_else(|| {
                    cx.new(|cx| {
                        PickerState::new_for_space(
                            self.color,
                            self.colors,
                            self.space,
                            self.appearance,
                            self.on_change.clone(),
                            cx,
                        )
                    })
                });
                (state.clone(), state)
            })
        });
        state.update(cx, |state, cx| {
            state.on_change = self.on_change;
            state.space = self.space;
            state.appearance = self.appearance;
            state.floating = self.floating;
            state.over_glass = self.over_glass;
            if state.dragging.is_none() && !state.frame_pending {
                if state.color != self.color || state.colors() != self.colors {
                    state.color = self.color;
                    state.handles = self
                        .colors
                        .colors()
                        .iter()
                        .enumerate()
                        .map(|(index, tint)| {
                            ColorHandle::new(
                                index as u64,
                                *tint,
                                index == self.colors.primary_index(),
                            )
                        })
                        .collect();
                    state.selected = self.colors.primary_index() as u64;
                    state.drag_palette = None;
                    state.arrange_followers(false, cx);
                    state.next_id = self.colors.colors().len() as u64;
                }
                state.sync_appearance(cx);
            }
        });
        state
    }
}

struct PickerState {
    color: WindowColor,
    preset_page: PresetPage,
    handles: Vec<ColorHandle>,
    selected: u64,
    next_id: u64,
    grab: (f32, f32),
    drag_palette: Option<PaletteDrag>,
    space: Option<photon_core::SpaceId>,
    appearance: PickerAppearance,
    noise: u8,
    transparency: u8,
    intensity_position: f32,
    on_change: TintChange,
    floating: bool,
    over_glass: bool,
    dragging: Option<Control>,
    frame_pending: bool,
    dirty_transparency: bool,
    dirty_tint: bool,
    dirty_noise: bool,
    _settings: Subscription,
    _release: Subscription,
    focus: [gpui::FocusHandle; 3],
}

impl PickerState {
    #[cfg(test)]
    fn new(
        color: WindowColor,
        colors: SpaceColors,
        on_change: TintChange,
        cx: &mut Context<Self>,
    ) -> Self {
        let space = Some(Settings::get(cx).spaces.active().id);
        Self::new_for_space(
            color,
            colors,
            space,
            PickerAppearance::default(),
            on_change,
            cx,
        )
    }

    /// The space the picker changes, or the default when it changes none.
    fn shown_space(space: Option<photon_core::SpaceId>, cx: &App) -> &photon_core::Space {
        let spaces = &Settings::get(cx).spaces;
        space
            .and_then(|space| spaces.get(space))
            .unwrap_or_else(|| spaces.active())
    }

    fn new_for_space(
        color: WindowColor,
        colors: SpaceColors,
        space: Option<photon_core::SpaceId>,
        appearance: PickerAppearance,
        on_change: TintChange,
        cx: &mut Context<Self>,
    ) -> Self {
        let owner = cx.entity_id();
        let release = cx.on_release(move |_, cx| AppearancePreview::clear(owner, cx));
        let transparency = appearance.transparency.unwrap_or_else(|| {
            AppearancePreview::transparency_in(Self::shown_space(space, cx), cx)
        });
        let noise = appearance
            .noise
            .unwrap_or_else(|| Settings::get(cx).background_noise_intensity);
        let mut state = Self {
            color,
            preset_page: PresetPage::default(),
            handles: colors
                .colors()
                .iter()
                .enumerate()
                .map(|(index, tint)| {
                    ColorHandle::new(index as u64, *tint, index == colors.primary_index())
                })
                .collect(),
            selected: colors.primary_index() as u64,
            next_id: colors.colors().len() as u64,
            grab: (0.0, 0.0),
            drag_palette: None,
            space,
            appearance,
            noise,
            transparency,
            intensity_position: f32::from(100 - transparency) / 100.0,
            on_change,
            floating: false,
            over_glass: false,
            dragging: None,
            frame_pending: false,
            dirty_transparency: false,
            dirty_tint: false,
            dirty_noise: false,
            _settings: cx.observe_global::<Settings>(|_, cx| cx.notify()),
            _release: release,
            focus: [cx.focus_handle(), cx.focus_handle(), cx.focus_handle()],
        };
        state.arrange_followers(false, cx);
        state
    }
    fn sync_appearance(&mut self, cx: &App) {
        if self.dragging.is_none() && !self.dirty_transparency {
            self.transparency = self.appearance.transparency.unwrap_or_else(|| {
                AppearancePreview::transparency_in(Self::shown_space(self.space, cx), cx)
            });
            self.intensity_position = f32::from(100 - self.transparency) / 100.0;
        }
        if self.dragging.is_none() && !self.dirty_noise {
            self.noise = self
                .appearance
                .noise
                .unwrap_or_else(|| Settings::get(cx).background_noise_intensity);
        }
    }
    fn resolved(&self) -> SpaceTint {
        self.handles
            .iter()
            .find(|handle| handle.id == self.selected && !handle.removing)
            .map(|handle| handle.tint)
            .unwrap_or_else(|| theme::preset_tint(WindowColor::System))
    }
    fn flush(&mut self, cx: &mut Context<Self>) {
        self.frame_pending = false;
        let tint = std::mem::take(&mut self.dirty_tint);
        let transparency = std::mem::take(&mut self.dirty_transparency);
        let noise = std::mem::take(&mut self.dirty_noise);
        if tint || transparency || noise {
            (self.on_change)(
                self.color,
                self.colors(),
                transparency.then_some(self.transparency),
                noise.then_some(self.noise),
                cx,
            );
        }
        AppearancePreview::clear(cx.entity_id(), cx);
    }
    fn publish(&self, cx: &mut Context<Self>) {
        if !self.dirty_tint && !self.dirty_transparency && !self.dirty_noise {
            return;
        }
        AppearancePreview {
            owner: Some(cx.entity_id()),
            tint: self.space.and_then(|space| {
                self.dirty_tint
                    .then_some((space, self.color, self.colors()))
            }),
            transparency: self.dirty_transparency.then_some(self.transparency),
            noise: self.dirty_noise.then_some(self.noise),
        }
        .publish(cx);
    }
    fn changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.frame_pending {
            self.frame_pending = true;
            let this = cx.entity().downgrade();
            window.on_next_frame(move |_, cx| {
                this.update(cx, |this, cx| {
                    if std::mem::take(&mut this.frame_pending) {
                        this.publish(cx);
                    }
                })
                .ok();
            });
        }
        cx.notify();
    }
    fn adjust(
        &mut self,
        control: Control,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let x = ((f32::from(position.x - bounds.origin.x)) / f32::from(bounds.size.width).max(1.0))
            .clamp(0.0, 1.0);
        let y = ((f32::from(position.y - bounds.origin.y))
            / f32::from(bounds.size.height).max(1.0))
        .clamp(0.0, 1.0);
        let previous = self.resolved();
        let previous_noise = self.noise;
        let previous_transparency = self.transparency;
        let mut tint = previous;
        match control {
            Control::Field => {
                let (x, y) = field_coordinates(position, bounds);
                self.dirty_tint |= self.drag_colors((x - self.grab.0, y - self.grab.1), cx);
                tint = self.resolved();
            }
            Control::Transparency => {
                let inset = metrics::PICKER_THUMB_MAX_WIDTH / 2.0;
                let width = f32::from(bounds.size.width);
                let fraction =
                    ((x * width - inset) / (width - inset * 2.0).max(1.0)).clamp(0.0, 1.0);
                self.intensity_position = fraction;
                self.transparency = 100 - (fraction * 100.0).round() as u8;
                if wave_detent(previous_transparency) != wave_detent(self.transparency) {
                    haptics::play(cx, Feedback::Detent(100 - self.transparency));
                }
            }
            Control::Noise => {
                if let Some(next) = noise_at(x, y) {
                    self.noise = next;
                }
                if previous_noise != self.noise {
                    haptics::play(cx, Feedback::Detent(self.noise * 5));
                }
            }
        }
        self.dirty_tint |= previous != tint;
        self.dirty_noise |= previous_noise != self.noise;
        self.dirty_transparency |= previous_transparency != self.transparency;
        if self.dirty_tint
            || previous != tint
            || previous_noise != self.noise
            || previous_transparency != self.transparency
        {
            self.changed(window, cx);
        } else if matches!(control, Control::Field | Control::Transparency) {
            // Pointer presentation must remain continuous between saved colour/percent steps.
            cx.notify();
        }
    }
    fn step(
        &mut self,
        control: Control,
        delta: i16,
        endpoint: Option<bool>,
        cx: &mut Context<Self>,
    ) {
        if self.colors().colors().is_empty() {
            return;
        }
        let mut tint = self.resolved();
        let (current, max) = match control {
            Control::Field => (tint.hue as i16, 359),
            Control::Transparency => (i16::from(100 - self.transparency), 100),
            Control::Noise => (
                i16::from(self.noise),
                i16::from(photon_core::BACKGROUND_NOISE_INTENSITY_MAX),
            ),
        };
        let next = endpoint
            .map(|end| if end { max } else { 0 })
            .unwrap_or(current + delta)
            .clamp(0, max);
        if next == current {
            return;
        }
        match control {
            Control::Field => {
                tint.hue = next as u16;
                self.set_tint(tint, true, cx);
            }
            Control::Transparency => self.transparency = 100 - next as u8,
            Control::Noise => self.noise = next as u8,
        }
        self.dirty_tint |= control == Control::Field;
        self.dirty_transparency |= control == Control::Transparency;
        self.dirty_noise |= control == Control::Noise;
        match control {
            Control::Transparency => haptics::play(cx, Feedback::Detent(100 - self.transparency)),
            Control::Noise => haptics::play(cx, Feedback::Detent(self.noise * 5)),
            Control::Field => {}
        }
        self.flush(cx);
        cx.notify();
    }
    fn nudge_lightness(&mut self, delta: i16, cx: &mut Context<Self>) {
        let mut tint = self.resolved();
        tint.lightness = (i16::from(tint.lightness) + delta).clamp(0, 100) as u8;
        self.set_tint(tint, true, cx);
        self.dirty_tint = true;
        self.flush(cx);
        cx.notify();
    }
    fn interaction(
        &self,
        control: Control,
        label: &'static str,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let disabled = self.colors().colors().is_empty() && control != Control::Field;
        let empty = self.colors().colors().is_empty();
        let focus = self.focus[match control {
            Control::Field => 0,
            Control::Transparency => 1,
            Control::Noise => 2,
        }]
        .clone();
        let focus_on_down = focus.clone();
        let down = cx.entity().downgrade();
        let up = down.clone();
        let moving = down.clone();
        let increment = down.clone();
        let decrement = down.clone();
        let remove = down.clone();
        let (value, max) = match control {
            Control::Field => (self.resolved().hue as f64, 359.0),
            Control::Transparency => (f64::from(100 - self.transparency), 100.0),
            Control::Noise => (
                f64::from(self.noise),
                f64::from(photon_core::BACKGROUND_NOISE_INTENSITY_MAX),
            ),
        };
        div()
            .id(label)
            .debug_selector(move || label.into())
            .role(Role::Slider)
            .aria_label(label)
            .aria_disabled(disabled)
            .tab_stop(!disabled)
            .when(control == Control::Field, |field| {
                field.aria_description(if empty {
                    "Click or press Enter to add the first colour."
                } else {
                    "Select a colour handle to edit it. Drag around the centre to change hue, and outward to brighten. Use [ and ] to select gradient colours; Delete or right-click removes the selected colour."
                })
            })
            .aria_numeric_value(value)
            .aria_min_numeric_value(0.0)
            .aria_max_numeric_value(max)
            .aria_numeric_value_step(1.0)
            .aria_value(format!("{value:.0}"))
            .track_focus(&focus)
            .tab_index(0)
            .focus_ring(palette)
            .cursor_crosshair()
            .on_mouse_down_all(move |event, phase, hitbox, window, cx| {
                if phase == DispatchPhase::Bubble
                    && event.button == MouseButton::Left
                    && hitbox.is_hovered(window)
                    && !disabled
                {
                    window.focus(&focus_on_down, cx);
                    window.capture_pointer(hitbox.id);
                    down.update(cx, |this, cx| {
                        this.dragging = Some(control);
                        let hit_handle = control == Control::Field
                            && this.begin_field(event.position, hitbox.bounds, cx);
                        if !hit_handle {
                            this.adjust(control, event.position, hitbox.bounds, window, cx);
                        } else if this.dirty_tint {
                            this.changed(window, cx);
                        }
                    })
                    .ok();
                    cx.stop_propagation();
                }
            })
            .on_mouse_down_all(move |event, phase, hitbox, window, cx| {
                if control == Control::Field
                    && phase == DispatchPhase::Bubble
                    && event.button == MouseButton::Right
                    && hitbox.is_hovered(window)
                {
                    remove
                        .update(cx, |this, cx| {
                            if let Some(id) = this.hit_handle(event.position, hitbox.bounds, cx) {
                                this.selected = id;
                                this.remove_color(cx);
                            }
                        })
                        .ok();
                    cx.stop_propagation();
                }
            })
            .on_mouse_move_all(move |event, phase, hitbox, window, cx| {
                if phase == DispatchPhase::Capture
                    && event.pressed_button == Some(MouseButton::Left)
                {
                    moving
                        .update(cx, |this, cx| {
                            if this.dragging == Some(control) {
                                this.adjust(control, event.position, hitbox.bounds, window, cx);
                            }
                        })
                        .ok();
                }
            })
            .on_mouse_up_all(move |event, _, _, window, cx| {
                if event.button == MouseButton::Left {
                    up.update(cx, |this, cx| {
                        if this.dragging == Some(control) {
                            this.dragging = None;
                            if control == Control::Field { this.release_handle(cx); }
                            window.release_pointer();
                            this.flush(cx);
                            cx.notify();
                        }
                    })
                    .ok();
                }
            })
            .on_key_down(cx.listener(move |this, event: &gpui::KeyDownEvent, _, cx| {
                if event.keystroke.modifiers.modified() {
                    return;
                }
                let key = event.keystroke.key.as_str();
                if this.colors().colors().is_empty() {
                    if control == Control::Field && matches!(key, "enter" | "space") {
                        this.add_color(cx);
                        cx.stop_propagation();
                    }
                    return;
                }
                if control == Control::Field {
                    match key {
                        "[" => this.select_relative(-1, cx),
                        "]" => this.select_relative(1, cx),
                        "up" => this.nudge_lightness(1, cx),
                        "down" => this.nudge_lightness(-1, cx),
                        "backspace" | "delete" => this.remove_color(cx),
                        _ => return,
                    }
                    cx.stop_propagation();
                    return;
                }
                let (delta, endpoint) = match key {
                    "left" => (-1, None),
                    "right" => (1, None),
                    "down" => (-1, None),
                    "up" => (1, None),
                    "home" => (0, Some(false)),
                    "end" => (0, Some(true)),
                    _ => return,
                };
                this.step(control, delta, endpoint, cx);
                cx.stop_propagation();
            }))
            .on_a11y_action(AccessibleAction::Increment, move |_, _, cx| {
                increment
                    .update(cx, |this, cx| this.step(control, 1, None, cx))
                    .ok();
            })
            .on_a11y_action(AccessibleAction::Decrement, move |_, _, cx| {
                decrement
                    .update(cx, |this, cx| this.step(control, -1, None, cx))
                    .ok();
            })
    }
}

impl Render for PickerState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_appearance(cx);
        let palette = theme::palette(window, cx);
        let colors = self.colors();
        let mut background_palette = palette;
        let ordered = theme::gradient_colors(colors);
        let chosen = ordered.colors();
        let empty = chosen.is_empty();
        if let Some(first) = chosen.first() {
            background_palette.window_tint = (theme::tint_swatch(*first) << 8) | 0xff;
            background_palette.window_tint_end =
                (theme::tint_swatch(*chosen.last().expect("a nonempty palette has a last colour"))
                    << 8)
                    | 0xff;
            background_palette.window_tint_middle =
                (chosen.len() == 3).then(|| (theme::tint_swatch(chosen[1]) << 8) | 0xff);
        } else {
            let default =
                (theme::tint_swatch(theme::default_tint(window.appearance())) << 8) | 0xff;
            background_palette.window_tint = default;
            background_palette.window_tint_end = default;
            background_palette.window_tint_middle = None;
        }
        // Over glass the tint lies on the glass as on the window, at the
        // theme's own transparency; a painted floating panel is opaque.
        let alpha = if self.floating && !self.over_glass {
            255
        } else {
            ((100 - self.transparency) as u32 * 255 + 50) / 100
        };
        background_palette.window_tint = (background_palette.window_tint & !0xff) | alpha;
        background_palette.window_tint_end = (background_palette.window_tint_end & !0xff) | alpha;
        background_palette.window_tint_middle = background_palette
            .window_tint_middle
            .map(|color| (color & !0xff) | alpha);
        let background = background_palette.background_average();
        // Keep picker text legible against the active tint in both glass and
        // opaque presentations. The tint can have very different contrast
        // from the selected page and menu appearance.
        let ink = theme::contrasting_text(background);
        let mut picker_palette = palette;
        picker_palette.text_primary = ink;
        let motion = Settings::motion(cx);
        let now = Instant::now();
        self.handles.retain(|handle| {
            !handle.removing || (motion.allows_motion() && handle.presence.is_running_at(now))
        });
        if motion.allows_motion() && self.handles.iter().any(|handle| handle.is_animating(now)) {
            window.request_animation_frame();
        }
        let mut painted = self
            .handles
            .iter()
            .map(|handle| handle.paint(handle.id == self.selected, now, &motion))
            .collect::<Vec<_>>();
        painted.sort_by_key(|handle| handle.selected);
        let selected_mode = Settings::get(cx).theme;
        let modes = [
            (
                ThemeMode::System,
                "System",
                include_bytes!("../../../assets/icons/theme-system.svg").as_slice(),
            ),
            (
                ThemeMode::Light,
                "Light",
                include_bytes!("../../../assets/icons/theme-light.svg").as_slice(),
            ),
            (
                ThemeMode::Dark,
                "Dark",
                include_bytes!("../../../assets/icons/theme-dark.svg").as_slice(),
            ),
        ];
        let mode_focus = RovingFocus::new(
            "theme-appearance-focus",
            modes
                .iter()
                .position(|(mode, _, _)| *mode == selected_mode)
                .unwrap_or(0),
            modes
                .iter()
                .map(|(mode, _, _)| {
                    let mode = *mode;
                    Rc::new(
                        Box::new(move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| {
                            Settings::update(cx, |settings| settings.theme = mode);
                            haptics::play(cx, Feedback::StateChange);
                        }) as super::ClickHandler,
                    )
                })
                .collect(),
            window,
            cx,
        );
        let buttons = modes
            .into_iter()
            .enumerate()
            .map(|(index, (mode, label, data))| {
                picker_control(ink)
                    .id(label)
                    .debug_selector(move || format!("theme-mode-{label}"))
                    .role(Role::RadioButton)
                    .aria_label(format!("{label} page and menu appearance"))
                    .aria_toggled(toggled(selected_mode == mode))
                    .track_focus(&mode_focus.handle(index))
                    .tab_index(index as isize)
                    .tab_stop(mode_focus.selected(index))
                    .focus_ring(picker_palette)
                    .owns_clicks()
                    .pressable()
                    .size(px(metrics::PICKER_MODE_SIZE))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(metrics::control_radius()))
                    .when(selected_mode == mode, |button| {
                        button.bg(rgb(ink).opacity(tokens().control_selected))
                    })
                    .tooltip(label_tooltip(format!("{label} pages and menus"), palette))
                    .on_click(mode_focus.on_click(index))
                    .on_key_down(mode_focus.on_key_down(index, RovingAxis::Horizontal))
                    .child(
                        svg()
                            .data(data)
                            .size(px(metrics::PICKER_ICON_SIZE))
                            .text_color(rgb(ink)),
                    )
            });
        let color_actions = [-1_i16, 1, 0]
            .into_iter()
            .map(|direction| {
                let label = if direction < 0 {
                    "Remove colour"
                } else if direction > 0 {
                    "Add colour"
                } else {
                    "Cycle colour harmony"
                };
                let count = self.colors().colors().len();
                let disabled = if direction < 0 {
                    count == 0
                } else if direction > 0 {
                    count == photon_core::MAX_SPACE_COLORS
                } else {
                    count < 2
                };
                picker_control(ink)
                    .id(label)
                    .debug_selector(move || label.into())
                    .role(Role::Button)
                    .aria_disabled(disabled)
                    .when(disabled, |button| {
                        button.opacity(
                            crate::platform::theme_config::theme()
                                .opacity
                                .disabled_control,
                        )
                    })
                    .when(empty && direction > 0, |button| {
                        button.opacity(
                            crate::platform::theme_config::theme()
                                .opacity
                                .disabled_control,
                        )
                    })
                    .aria_label(label)
                    .tab_index(0)
                    .size(px(metrics::PICKER_MODE_SIZE))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(ink))
                    .text_size(px(metrics::PICKER_ICON_SIZE))
                    .focus_ring(picker_palette)
                    .owns_clicks()
                    .pressable()
                    .tooltip(label_tooltip(label, palette))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if direction < 0 {
                            this.remove_color(cx)
                        } else if direction > 0 {
                            this.add_color(cx)
                        } else {
                            this.cycle_harmony(cx)
                        }
                    }))
                    .on_key_down(cx.listener(move |this, event: &gpui::KeyDownEvent, _, cx| {
                        if !event.keystroke.modifiers.modified()
                            && matches!(event.keystroke.key.as_str(), "enter" | "space")
                        {
                            if direction < 0 {
                                this.remove_color(cx)
                            } else if direction > 0 {
                                this.add_color(cx)
                            } else {
                                this.cycle_harmony(cx)
                            };
                            cx.stop_propagation();
                        }
                    }))
                    .child(
                        svg()
                            .data(if direction < 0 {
                                include_bytes!("../../../assets/icons/lucide/minus.svg").as_slice()
                            } else if direction > 0 {
                                include_bytes!("../../../assets/icons/lucide/plus.svg").as_slice()
                            } else {
                                include_bytes!("../../../assets/icons/lucide/rotate-cw.svg")
                                    .as_slice()
                            })
                            .size(px(metrics::PICKER_ICON_SIZE))
                            .text_color(rgb(ink)),
                    )
            })
            .collect::<Vec<_>>();
        let panel_radius = metrics::theme_accent_panel_radius();
        let field = self
            .interaction(
                Control::Field,
                "Theme colour: left/right hue, up/down brightness",
                picker_palette,
                cx,
            )
            .relative()
            .w_full()
            .aspect_ratio(1.0)
            .rounded(px(panel_radius))
            .overflow_hidden()
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| paint_field(bounds, ink, &painted, window),
                )
                .size_full(),
            )
            .children(empty.then(|| {
                div()
                    .id("theme-picker-empty-prompt")
                    .debug_selector(|| "theme-picker-empty-prompt".into())
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(metrics::INTERNAL_PAGE_BODY_SIZE))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(rgb(ink))
                    .aria_hidden()
                    .child("Click to pick a colour for this space")
            }))
            .child(
                h_stack()
                    .id("theme-appearance-modes")
                    .absolute()
                    .top(px(metrics::PICKER_PADDING))
                    .left_0()
                    .right_0()
                    .justify_center()
                    .gap(px(metrics::PICKER_SWATCH_GAP))
                    .role(Role::RadioGroup)
                    .aria_label("Page and menu appearance")
                    .children(buttons),
            )
            .child(
                h_stack()
                    .absolute()
                    .bottom(px(metrics::PICKER_PADDING))
                    .left_0()
                    .right_0()
                    .justify_center()
                    .gap(px(metrics::PICKER_SWATCH_GAP))
                    .children(color_actions),
            );
        let preset_focus = RovingFocus::new(
            "theme-preset-focus",
            WindowColor::ALL
                .iter()
                .position(|color| *color == self.color)
                .unwrap_or(0),
            WindowColor::ALL
                .into_iter()
                .map(|color| {
                    Rc::new(
                        Box::new(cx.listener(move |this, _, _, cx| this.choose(color, cx)))
                            as super::ClickHandler,
                    )
                })
                .collect(),
            window,
            cx,
        );
        let swatches: Vec<_> = WindowColor::ALL
            .into_iter()
            .enumerate()
            .map(|(index, color)| {
                let preset = self.preset_page.colors(color);
                let selected = self.color == color
                    && colors.primary() == preset.primary()
                    && colors.colors().len() == preset.colors().len()
                    && preset
                        .colors()
                        .iter()
                        .all(|tint| colors.colors().contains(tint));
                let chosen = theme::gradient_colors(preset);
                let label = if self.preset_page == PresetPage::Grayscale {
                    format!("Gray {}%", chosen.colors()[0].lightness)
                } else {
                    format!("{} · {}", color.name(), self.preset_page.label())
                };
                let mut swatch_palette = palette;
                swatch_palette.window_tint = (theme::tint_swatch(chosen.colors()[0]) << 8) | 0xff;
                swatch_palette.window_tint_end =
                    (theme::tint_swatch(*chosen.colors().last().unwrap()) << 8) | 0xff;
                swatch_palette.window_tint_middle = (chosen.colors().len() == 3)
                    .then(|| (theme::tint_swatch(chosen.colors()[1]) << 8) | 0xff);
                div()
                    .id((ElementId::from("theme-preset"), color.name()))
                    .debug_selector(move || format!("theme-preset-{}", color.name()))
                    .role(Role::RadioButton)
                    .aria_label(label.clone())
                    .aria_toggled(toggled(selected && !empty))
                    .aria_disabled(empty)
                    .when(empty, |swatch| {
                        swatch.opacity(
                            crate::platform::theme_config::theme()
                                .opacity
                                .disabled_control,
                        )
                    })
                    .track_focus(&preset_focus.handle(index))
                    .tab_index(index as isize)
                    .tab_stop(!empty && preset_focus.selected(index))
                    .focus_ring(picker_palette)
                    .owns_clicks()
                    .pressable()
                    .size(px(metrics::PICKER_SWATCH_SIZE))
                    .rounded_full()
                    .border_2()
                    .border_color(if selected {
                        gpui::rgb_to_hsla(rgb(ink))
                    } else {
                        gpui::transparent_black()
                    })
                    .relative()
                    .overflow_hidden()
                    .bg(swatch_palette.window_background())
                    .children(swatch_palette.gradient_overlay(metrics::PICKER_SWATCH_SIZE / 2.0))
                    .tooltip(label_tooltip(label, palette))
                    .on_click(preset_focus.on_click(index))
                    .on_key_down(preset_focus.on_key_down(index, RovingAxis::Any))
            })
            .collect();
        let preset_arrows = [-1_i16, 1].map(|direction| {
            let label = if direction < 0 {
                "Previous preset page"
            } else {
                "Next preset page"
            };
            picker_control(ink)
                .id(label)
                .debug_selector(move || label.into())
                .role(Role::Button)
                .aria_label(label)
                .aria_disabled(empty)
                .tab_stop(!empty)
                .when(empty, |button| {
                    button.opacity(
                        crate::platform::theme_config::theme()
                            .opacity
                            .disabled_control,
                    )
                })
                .tab_index(0)
                .focus_ring(picker_palette)
                .owns_clicks()
                .pressable()
                .tooltip(label_tooltip(self.preset_page.label(), palette))
                .on_click(cx.listener(move |this, _, _, cx| this.change_preset_page(direction, cx)))
                .on_key_down(cx.listener(move |this, event: &gpui::KeyDownEvent, _, cx| {
                    if !event.keystroke.modifiers.modified()
                        && matches!(event.keystroke.key.as_str(), "enter" | "space")
                    {
                        this.change_preset_page(direction, cx);
                        cx.stop_propagation();
                    }
                }))
                .child(
                    svg()
                        .data(if direction < 0 {
                            include_bytes!("../../../assets/icons/lucide/arrow-left.svg").as_slice()
                        } else {
                            include_bytes!("../../../assets/icons/lucide/arrow-right.svg")
                                .as_slice()
                        })
                        .size(px(metrics::PICKER_ICON_SIZE))
                        .text_color(rgb(ink)),
                )
        });
        let [previous_presets, next_presets] = preset_arrows;
        let intensity = 100 - self.transparency;
        let intensity_position = self.intensity_position;
        let transparency_slider = self
            .interaction(Control::Transparency, "Intensity", picker_palette, cx)
            .flex_1()
            .min_w_0()
            .h(px(metrics::PICKER_FOOTER_HEIGHT))
            .cursor_col_resize()
            .when(empty, |slider| {
                slider.opacity(
                    crate::platform::theme_config::theme()
                        .opacity
                        .disabled_control,
                )
            })
            .tooltip(label_tooltip(format!("Intensity {intensity}%"), palette))
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        paint_wave(bounds, intensity_position, ink, !empty, window)
                    },
                )
                .size_full(),
            );
        let noise = self.noise;
        let knob = self
            .interaction(Control::Noise, "Grain intensity", picker_palette, cx)
            .size(px(metrics::PICKER_KNOB_SIZE))
            .flex_shrink_0()
            .when(empty, |knob| {
                knob.opacity(
                    crate::platform::theme_config::theme()
                        .opacity
                        .disabled_control,
                )
            })
            .tooltip(label_tooltip(format!("Grain {}%", noise * 5), palette))
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| paint_knob(bounds, noise, ink, !empty, window),
                )
                .size_full(),
            );
        let mut panel = v_stack()
            .w_full()
            .max_w(px(metrics::PICKER_WIDTH))
            .p(px(metrics::PICKER_PADDING))
            .gap(px(metrics::PICKER_GAP))
            .rounded(px(panel_radius))
            .relative()
            // The theme's tint, live as it is edited, over native glass too.
            .bg(background_palette.window_background())
            .children(background_palette.gradient_overlay(panel_radius))
            // Native glass draws its own edge.
            .when(!self.over_glass, |panel| {
                panel
                    .border_1()
                    .border_color(rgb(ink).opacity(tokens().track))
            })
            .children(
                (self.noise > 0)
                    .then(|| super::background_noise::overlay(self.noise, panel_radius)),
            );
        panel = panel
            .child(field)
            .child(
                h_stack()
                    .id("theme-colour-presets")
                    .role(Role::RadioGroup)
                    .aria_label(format!("{} presets", self.preset_page.label()))
                    .w_full()
                    .items_center()
                    .justify_between()
                    .child(previous_presets)
                    .children(swatches)
                    .child(next_presets),
            )
            .child(
                h_stack()
                    .w_full()
                    .items_center()
                    .gap(px(metrics::PICKER_GAP))
                    .child(transparency_slider)
                    .child(knob),
            );
        if self.floating && !self.over_glass {
            panel.elevated(Elevation::High)
        } else {
            panel
        }
    }
}
