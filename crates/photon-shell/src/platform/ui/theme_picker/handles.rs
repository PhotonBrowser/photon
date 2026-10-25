//! Independent colour handles and their interruptible spring presentation.

use super::super::motion::{MotionConfig, THEME_PICKER_SPRING};
use super::paint::PaintedHandle;
use super::*;
use photon_core::MAX_SPACE_COLORS;

pub(super) struct ColorHandle {
    pub id: u64,
    pub tint: SpaceTint,
    pub x: Tween,
    pub y: Tween,
    pub scale: Tween,
    pub presence: Tween,
    pub removing: bool,
}
impl ColorHandle {
    pub fn new(id: u64, tint: SpaceTint, selected: bool) -> Self {
        let (x, y) = field_position(tint);
        Self {
            id,
            tint,
            x: Tween::at_with_curve(x, THEME_PICKER_SPRING),
            y: Tween::at_with_curve(y, THEME_PICKER_SPRING),
            scale: Tween::at_with_curve(
                if selected {
                    1.0
                } else {
                    metrics::PICKER_SECONDARY_HANDLE_SCALE
                },
                THEME_PICKER_SPRING,
            ),
            presence: Tween::at_with_curve(1.0, THEME_PICKER_SPRING),
            removing: false,
        }
    }
    pub fn is_animating(&self, now: Instant) -> bool {
        self.x.is_running_at(now)
            || self.y.is_running_at(now)
            || self.scale.is_running_at(now)
            || self.presence.is_running_at(now)
    }
    pub fn position(&self, now: Instant, config: &MotionConfig) -> (f32, f32) {
        (
            self.x.value_with_config(now, config).clamp(0.0, 1.0),
            self.y.value_with_config(now, config).clamp(0.0, 1.0),
        )
    }

    pub fn paint(&self, selected: bool, now: Instant, config: &MotionConfig) -> PaintedHandle {
        let (x, y) = self.position(now, config);
        PaintedHandle {
            x,
            y,
            scale: self.scale.value_with_config(now, config).max(0.0),
            opacity: self.presence.value_with_config(now, config).clamp(0.0, 1.0),
            tint: self.tint,
            selected,
        }
    }
}
pub(super) struct PaletteDrag {
    center: (f32, f32),
    colors: Vec<(u64, SpaceTint)>,
}

impl PickerState {
    pub(super) fn colors(&self) -> SpaceColors {
        let mut colors = [SpaceTint::default(); MAX_SPACE_COLORS];
        let mut count = 0;
        let mut primary = 0;
        for handle in self
            .handles
            .iter()
            .filter(|handle| !handle.removing)
            .take(MAX_SPACE_COLORS)
        {
            if handle.id == self.selected {
                primary = count;
            }
            colors[count] = handle.tint;
            count += 1;
        }
        SpaceColors::from_colors(&colors[..count]).with_primary(primary)
    }
    pub(super) fn selected_handle(&mut self) -> &mut ColorHandle {
        self.handles
            .iter_mut()
            .find(|handle| handle.id == self.selected && !handle.removing)
            .expect("the picker always keeps a selected colour")
    }
    pub(super) fn arrange_followers(&mut self, animate: bool, cx: &App) {
        let motion = Settings::motion(cx);
        for handle in self
            .handles
            .iter_mut()
            .filter(|h| !h.removing && h.id != self.selected)
        {
            let (x, y) = field_position(handle.tint);
            if animate {
                handle.x.animate_to(x, Speed::Standard, &motion);
                handle.y.animate_to(y, Speed::Standard, &motion);
            } else {
                handle.x.jump_to(x);
                handle.y.jump_to(y);
            }
        }
    }

    fn promote(&mut self, id: u64, held: bool, cx: &mut Context<Self>) -> (f32, f32) {
        if self.selected != id {
            self.selected = id;
            self.dirty_tint = true;
            haptics::play(cx, Feedback::Alignment);
        }
        let motion = Settings::motion(cx);
        let now = Instant::now();
        let handle = self.selected_handle();
        let center = handle.position(now, &motion);
        handle.x.jump_to(center.0);
        handle.y.jump_to(center.1);
        self.arrange_followers(true, cx);
        self.resize_handles(held, cx);
        center
    }
    fn drag_snapshot(&self, center: (f32, f32)) -> PaletteDrag {
        PaletteDrag {
            center,
            colors: self
                .handles
                .iter()
                .filter(|handle| !handle.removing)
                .map(|handle| (handle.id, handle.tint))
                .collect(),
        }
    }
    pub(super) fn drag_colors(&mut self, center: (f32, f32), cx: &App) -> bool {
        if self.drag_palette.is_none() {
            self.drag_palette = Some(self.drag_snapshot(field_position(self.resolved())));
        }
        let anchor = self
            .drag_palette
            .as_ref()
            .expect("a colour drag has a palette snapshot");
        let center = paint::clamp_field(center.0, center.1);
        let mut before = tint_at(anchor.center.0, anchor.center.1);
        if (anchor.center.0 - 0.5).hypot(anchor.center.1 - 0.5) < metrics::PICKER_HUE_DEAD_ZONE {
            before.hue = anchor
                .colors
                .iter()
                .find(|(id, _)| *id == self.selected)
                .map_or(before.hue, |(_, tint)| tint.hue);
        }
        let after = tint_at(center.0, center.1);
        // Hue is undefined at the centre. Keep the drag's hue until it leaves the dead zone.
        let hue = if (center.0 - 0.5).hypot(center.1 - 0.5) < metrics::PICKER_HUE_DEAD_ZONE {
            0.0
        } else {
            (f32::from(after.hue) - f32::from(before.hue) + 180.0).rem_euclid(360.0) - 180.0
        };
        let lightness = f32::from(after.lightness) - f32::from(before.lightness);
        let mut changed = false;
        for handle in self.handles.iter_mut().filter(|handle| !handle.removing) {
            if let Some((_, original)) = anchor.colors.iter().find(|(id, _)| *id == handle.id) {
                let next = shifted(*original, hue, lightness);
                changed |= next != handle.tint;
                handle.tint = next;
            }
            if handle.id == self.selected {
                handle.x.jump_to(center.0);
                handle.y.jump_to(center.1);
            }
        }
        self.arrange_followers(true, cx);
        changed
    }
    pub(super) fn set_tint(&mut self, tint: SpaceTint, animate: bool, cx: &App) {
        if self.colors().colors().is_empty() {
            return;
        }
        let previous = self.resolved();
        let hue = (f32::from(tint.hue) - f32::from(previous.hue) + 180.0).rem_euclid(360.0) - 180.0;
        let lightness = f32::from(tint.lightness) - f32::from(previous.lightness);
        let motion = Settings::motion(cx);
        let center = field_position(tint);
        for handle in self.handles.iter_mut().filter(|handle| !handle.removing) {
            if handle.id == self.selected {
                handle.tint = tint;
                if animate {
                    handle.x.animate_to(center.0, Speed::Standard, &motion);
                    handle.y.animate_to(center.1, Speed::Standard, &motion);
                } else {
                    handle.x.jump_to(center.0);
                    handle.y.jump_to(center.1);
                }
            } else {
                handle.tint = shifted(handle.tint, hue, lightness);
            }
        }
        self.arrange_followers(animate, cx);
    }
    pub(super) fn resize_handles(&mut self, held: bool, cx: &App) {
        let motion = Settings::motion(cx);
        for handle in self.handles.iter_mut().filter(|handle| !handle.removing) {
            let scale = if handle.id == self.selected {
                if held {
                    metrics::PICKER_HELD_HANDLE_SCALE
                } else {
                    1.0
                }
            } else {
                metrics::PICKER_SECONDARY_HANDLE_SCALE
            };
            handle.scale.animate_to(scale, Speed::Standard, &motion);
        }
    }
    pub(super) fn select_relative(&mut self, offset: isize, cx: &mut Context<Self>) {
        let handles: Vec<_> = self
            .handles
            .iter()
            .filter(|handle| !handle.removing)
            .map(|handle| handle.id)
            .collect();
        if handles.is_empty() {
            return;
        }
        let index = handles
            .iter()
            .position(|id| *id == self.selected)
            .unwrap_or(0);
        let id = handles[(index as isize + offset).rem_euclid(handles.len() as isize) as usize];
        self.promote(id, false, cx);
        self.flush(cx);
        cx.notify();
    }
    pub(super) fn hit_handle(
        &self,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        cx: &App,
    ) -> Option<u64> {
        let (x, y) = field_coordinates(position, bounds);
        let width = (f32::from(bounds.size.width) - metrics::PICKER_HANDLE_RADIUS * 2.0).max(1.0);
        let height = (f32::from(bounds.size.height) - metrics::PICKER_HANDLE_RADIUS * 2.0).max(1.0);
        let motion = Settings::motion(cx);
        let now = Instant::now();
        self.handles
            .iter()
            .filter(|h| !h.removing)
            .filter_map(|h| {
                let (hx, hy) = h.position(now, &motion);
                let distance = ((x - hx) * width).hypot((y - hy) * height);
                let radius = metrics::PICKER_HANDLE_RADIUS
                    * h.scale.value_with_config(now, &motion)
                    + metrics::PICKER_HANDLE_HIT_SLOP;
                (distance <= radius).then_some((distance, h.id))
            })
            .min_by(|a, b| {
                a.0.total_cmp(&b.0)
                    .then_with(|| (a.1 != self.selected).cmp(&(b.1 != self.selected)))
            })
            .map(|(_, id)| id)
    }
    pub(super) fn begin_field(
        &mut self,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        let (x, y) = field_coordinates(position, bounds);
        if self.colors().colors().is_empty() {
            let center = paint::clamp_field(x, y);
            self.create_first(tint_at(center.0, center.1), true, cx);
            self.drag_palette = Some(self.drag_snapshot(center));
            return false;
        }
        let hit = self.hit_handle(position, bounds, cx);
        let hit_handle = hit.is_some();
        self.grab = if let Some(id) = hit {
            let (px, py) = self.promote(id, true, cx);
            self.drag_palette = Some(self.drag_snapshot((px.clamp(0.0, 1.0), py.clamp(0.0, 1.0))));
            (x - px, y - py)
        } else {
            self.drag_palette = Some(self.drag_snapshot(field_position(self.resolved())));
            (0.0, 0.0)
        };
        self.resize_handles(true, cx);
        cx.notify();
        hit_handle
    }
    pub(super) fn release_handle(&mut self, cx: &mut Context<Self>) {
        self.grab = (0.0, 0.0);
        self.drag_palette = None;
        if !self.colors().colors().is_empty() {
            // A handle grabbed mid-spring tracks the pointer without jumping, then
            // settles back onto the exact coordinate for its committed colour.
            self.set_tint(self.resolved(), true, cx);
        }
        self.resize_handles(false, cx);
    }
    fn create_first(&mut self, tint: SpaceTint, held: bool, cx: &mut Context<Self>) {
        let motion = Settings::motion(cx);
        let mut handle = ColorHandle::new(self.next_id, tint, true);
        self.next_id += 1;
        self.selected = handle.id;
        handle.scale.jump_to(0.0);
        handle.presence.jump_to(0.0);
        handle.scale.animate_to(
            if held {
                metrics::PICKER_HELD_HANDLE_SCALE
            } else {
                1.0
            },
            Speed::Standard,
            &motion,
        );
        handle.presence.animate_to(1.0, Speed::Standard, &motion);
        self.handles.retain(|handle| !handle.removing);
        self.handles.push(handle);
        self.color = WindowColor::Blue;
        self.dirty_tint = true;
        self.transparency = AppearancePreview::subtle_transparency();
        self.intensity_position = f32::from(100 - self.transparency) / 100.0;
        self.dirty_transparency = true;
        cx.notify();
    }
    pub(super) fn add_color(&mut self, cx: &mut Context<Self>) {
        if self.colors().colors().is_empty() {
            self.create_first(SpaceTint::default(), false, cx);
            self.flush(cx);
            haptics::play(cx, Feedback::StateChange);
            return;
        }
        if self.colors().colors().len() >= MAX_SPACE_COLORS {
            return;
        }
        let motion = Settings::motion(cx);
        let now = Instant::now();
        let selected = self.selected_handle();
        let origin = (
            selected.x.value_with_config(now, &motion),
            selected.y.value_with_config(now, &motion),
        );
        let mut tint = selected.tint;
        let offsets = super::harmony::default_offsets(self.colors().colors().len() + 1);
        tint.hue = (tint.hue + offsets[offsets.len() - 1]) % 360;
        if tint.saturation == 0 {
            tint.saturation = (tokens().field_saturation * 100.0).round() as u8;
        }
        let mut handle = ColorHandle::new(self.next_id, tint, false);
        self.next_id += 1;
        handle.x.jump_to(origin.0);
        handle.y.jump_to(origin.1);
        handle.scale.jump_to(0.0);
        handle.presence.jump_to(0.0);
        handle.presence.animate_to(1.0, Speed::Standard, &motion);
        if self.handles.len() >= MAX_SPACE_COLORS * 2
            && let Some(index) = self.handles.iter().position(|handle| handle.removing)
        {
            self.handles.remove(index);
        }
        self.handles.push(handle);
        self.apply_harmony(offsets, cx);
        self.resize_handles(false, cx);
        self.dirty_tint = true;
        self.flush(cx);
        haptics::play(cx, Feedback::StateChange);
        cx.notify();
    }
    pub(super) fn remove_color(&mut self, cx: &mut Context<Self>) {
        if self.colors().colors().is_empty() {
            return;
        }
        let motion = Settings::motion(cx);
        let handle = self.selected_handle();
        handle.removing = true;
        handle.scale.animate_to(0.0, Speed::Standard, &motion);
        handle.presence.animate_to(0.0, Speed::Standard, &motion);
        self.selected = self
            .handles
            .iter()
            .rev()
            .find(|handle| !handle.removing)
            .map(|handle| handle.id)
            .unwrap_or(u64::MAX);
        if self.colors().colors().is_empty() {
            self.color = WindowColor::System;
            self.transparency = AppearancePreview::subtle_transparency();
            self.intensity_position = f32::from(100 - self.transparency) / 100.0;
            self.dirty_transparency = true;
        }
        if !self.colors().colors().is_empty() {
            self.promote(self.selected, false, cx);
        }
        self.drag_palette = None;
        self.resize_handles(false, cx);
        self.dirty_tint = true;
        self.flush(cx);
        haptics::play(cx, Feedback::StateChange);
        cx.notify();
    }
}
fn shifted(original: SpaceTint, hue: f32, lightness: f32) -> SpaceTint {
    SpaceTint {
        hue: (f32::from(original.hue) + hue).round().rem_euclid(360.0) as u16,
        lightness: (f32::from(original.lightness) + lightness)
            .round()
            .clamp(0.0, 100.0) as u8,
        ..original
    }
}
pub(super) fn field_coordinates(position: Point<Pixels>, bounds: Bounds<Pixels>) -> (f32, f32) {
    let radius = metrics::PICKER_HANDLE_RADIUS;
    (
        ((f32::from(position.x - bounds.origin.x) - radius)
            / (f32::from(bounds.size.width) - radius * 2.0).max(1.0)),
        ((f32::from(position.y - bounds.origin.y) - radius)
            / (f32::from(bounds.size.height) - radius * 2.0).max(1.0)),
    )
}
