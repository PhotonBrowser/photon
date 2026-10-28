//! Tabs on the move: dragged along the strip or the sidebar to reorder,
//! dropped on favourites or the Today section, or carried across to
//! another window, with the tabs around them making room as they go.

use gpui::{AnyDrag, App, AppContext, Context, DispatchPhase, Hitbox, MouseMoveEvent, Window};
use photon_core::WindowSession;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use super::super::super::settings::Settings;
use super::super::super::tabs::{
    DraggedTab, TabDragAxis, TabDragPayload, TabDragPreview, TabDrop, TabPrepaint,
};
use super::super::super::workspace_layout::WorkspaceLayout;
use super::super::BrowserWindow;
use super::super::content::TabContent;
use super::super::registry::BrowserWindowRegistry;

static NEXT_TAB_BUTTON_MOTION_ID: AtomicU64 = AtomicU64::new(1);

impl BrowserWindow {
    pub(in super::super::super) fn bind_tab_reorder_groups(&self, cx: &Context<Self>) {
        let browser = cx.entity().downgrade();
        for axis in [TabDragAxis::Horizontal, TabDragAxis::Vertical] {
            for pinned in [true, false] {
                let browser = browser.clone();
                self.tab_reorder_groups
                    .group(axis, pinned)
                    .clone()
                    .on_keyboard_change(move |order, _, cx| {
                        let order = order.to_vec();
                        browser
                            .update(cx, |this, cx| {
                                this.apply_tab_reorder(axis, pinned, &order, cx);
                            })
                            .ok();
                    });
            }
        }
    }

    fn apply_tab_reorder(
        &mut self,
        axis: TabDragAxis,
        pinned: bool,
        order: &[u64],
        cx: &mut Context<Self>,
    ) {
        let current: Vec<u64> = match axis {
            TabDragAxis::Horizontal => self
                .tabs
                .iter()
                .filter(|tab| tab.pin.is_some() == pinned)
                .map(|tab| tab.id)
                .collect(),
            TabDragAxis::Vertical => self
                .listed_tabs(cx)
                .into_iter()
                .filter(|&index| self.tabs[index].pin.is_some() == pinned)
                .map(|index| self.tabs[index].id)
                .collect(),
        };
        if current.len() != order.len()
            || current.iter().any(|id| !order.contains(id))
            || current == order
        {
            return;
        }

        let Some(first) = current
            .iter()
            .zip(order)
            .position(|(left, right)| left != right)
        else {
            return;
        };
        let last = current
            .iter()
            .zip(order)
            .rposition(|(left, right)| left != right)
            .expect("first mismatch exists");
        let moved_id = if current[first] == order[last] {
            current[first]
        } else if current[last] == order[first] {
            current[last]
        } else {
            return;
        };

        self.move_tab_to_visible_order(moved_id, order);
        self.tab_position_animations
            .retain(|id, _| !order.contains(id));
        cx.notify();
    }

    pub(in super::super::super) fn update_tab_drag(
        &mut self,
        event: &MouseMoveEvent,
        phase: DispatchPhase,
        hitbox: &Hitbox,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if phase != DispatchPhase::Capture || !event.dragging() {
            return;
        }
        let position = event.position;
        self.tab_container_bounds = Some(hitbox.bounds);
        let current_window = window.window_handle();

        if let Some(dragged) = cx
            .active_drag()
            .and_then(|drag| drag.value.downcast_ref::<DraggedTab>())
            .cloned()
            && dragged.source != current_window
        {
            self.update_incoming_drag(&dragged, position, current_window, cx);
            return;
        }

        let Some(layout) = self.tab_drag_layout.as_ref() else {
            return;
        };
        let layout_id = layout.id;
        if layout.axis != self.tab_axis(cx) {
            self.clear_previous_target(cx);
            self.clear_favourites_drag_hover(current_window, layout_id, cx);
            let _ = cx.stop_active_drag(window);
            window.release_pointer();
            self.tab_drag_layout = None;
            Self::notify_other_tab_drop_targets(cx, current_window);
            cx.notify();
            return;
        }
        if layout.phase == super::super::TabDragPhase::PendingDrag {
            if (position - layout.pointer_start).magnitude()
                < f64::from(super::super::super::metrics::TAB_DRAG_THRESHOLD)
            {
                return;
            }

            if self.tab_index_by_id(layout_id).is_none() {
                self.tab_drag_layout = None;
                return;
            }
            let axis = layout.axis;
            let pinned = layout.pinned;
            let bounds = self
                .tab_geometry
                .get(&layout_id)
                .copied()
                .unwrap_or(layout.tab_bounds);
            let dragged = DraggedTab {
                source: window.window_handle(),
                payload: TabDragPayload {
                    id: layout_id,
                    size: match layout.axis {
                        TabDragAxis::Horizontal => f32::from(bounds.size.width),
                        TabDragAxis::Vertical => f32::from(bounds.size.height),
                    },
                    pinned: layout.pinned,
                },
            };
            let offset = layout.grab_offset;
            let reorder = self.tab_reorder_groups.group(axis, pinned);
            let measured_reorder = reorder.start_drag(
                &layout_id,
                [f64::from(position.x), f64::from(position.y)],
                window,
                cx,
            );
            if measured_reorder {
                for id in reorder.order().iter() {
                    self.tab_position_animations.remove(id);
                }
            }
            let preview = cx.new(|_| TabDragPreview);
            if let Some(layout) = self.tab_drag_layout.as_mut() {
                layout.phase = super::super::TabDragPhase::Reordering;
                layout.pointer_position = position;
                layout.measured_reorder = measured_reorder;
            }
            self.suppress_tab_click_until = Some(Instant::now() + Duration::from_millis(250));
            cx.start_drag(AnyDrag::new(dragged, preview).offset(offset));
            window.capture_pointer(hitbox.id);
            Self::notify_other_tab_drop_targets(cx, current_window);
            cx.notify();
            window.refresh();
            return;
        }

        let Some(dragged) = cx
            .active_drag()
            .and_then(|drag| drag.value.downcast_ref::<DraggedTab>())
            .cloned()
        else {
            return;
        };
        if dragged.source != current_window || dragged.id != layout.id {
            return;
        }

        let axis = layout.axis;
        let previous_position = layout.pointer_position;
        if let Some(layout) = self.tab_drag_layout.as_mut() {
            layout.pointer_position = position;
        }
        let source_bounds = window.bounds();
        let source_content_origin = window.inner_window_bounds().get_bounds().origin;
        let global_position = source_content_origin + position;
        let previous_global_position = source_content_origin + previous_position;
        if let Some((target, kind, target_position)) = self.external_drop_target(
            global_position,
            previous_global_position,
            current_window,
            source_bounds,
            cx,
        ) {
            match kind {
                ExternalDropTargetKind::Favourites => {
                    self.clear_favourites_drag_hover(current_window, dragged.id, cx);
                    let target_handle = target.into();
                    let already_targeting = self
                        .tab_drag_layout
                        .as_ref()
                        .is_some_and(|layout| layout.target_window == Some(target_handle));
                    if !already_targeting {
                        self.clear_previous_target(cx);
                    }
                    target
                        .update(cx, |browser, _, cx| {
                            browser.set_favourites_drag_hover(current_window, dragged.id, cx);
                        })
                        .ok();
                    let changed = if let Some(layout) = self.tab_drag_layout.as_mut() {
                        let changed = layout.phase
                            != super::super::TabDragPhase::DraggingOverFavourites
                            || layout.target_window != Some(target_handle);
                        layout.phase = super::super::TabDragPhase::DraggingOverFavourites;
                        layout.target_window = Some(target_handle);
                        changed
                    } else {
                        false
                    };
                    if changed {
                        cx.notify();
                    }
                    return;
                }
                ExternalDropTargetKind::TabStrip => {
                    self.clear_favourites_drag_hover(current_window, dragged.id, cx);
                    self.set_cross_window_target(target, &dragged, target_position, cx);
                    return;
                }
                ExternalDropTargetKind::WindowBody => {
                    self.clear_favourites_drag_hover(current_window, dragged.id, cx);
                    self.clear_previous_target(cx);
                    self.set_tab_drag_phase(super::super::TabDragPhase::OutsideTarget, cx);
                    return;
                }
            }
        }
        self.clear_previous_target(cx);
        let in_favourites = self.is_favourites_drop_position(position, cx);
        if in_favourites {
            self.set_favourites_drag_hover(current_window, dragged.id, cx);
            self.set_tab_drag_phase(super::super::TabDragPhase::DraggingOverFavourites, cx);
            return;
        }
        self.clear_favourites_drag_hover(current_window, dragged.id, cx);

        let in_tab_container = self
            .tab_container_bounds
            .is_some_and(|bounds| bounds.contains(&position));
        if in_tab_container {
            if let Some(layout) = self.tab_drag_layout.as_mut() {
                layout.phase = super::super::TabDragPhase::Reordering;
            }
            let measured_reorder = self
                .tab_drag_layout
                .as_ref()
                .is_some_and(|layout| layout.measured_reorder);
            if measured_reorder {
                cx.notify();
            } else {
                self.reorder_tab_at_pointer(dragged.id, axis, position, cx, window);
            }
            return;
        }

        self.set_tab_drag_phase(super::super::TabDragPhase::OutsideTarget, cx);
    }

    fn reorder_tab_at_pointer(
        &mut self,
        dragged_id: u64,
        axis: TabDragAxis,
        position: gpui::Point<gpui::Pixels>,
        cx: &mut Context<Self>,
        _window: &mut Window,
    ) {
        let Some(dragged_index) = self.tab_index_by_id(dragged_id) else {
            return;
        };
        let pinned = self.tabs[dragged_index].pin.is_some();
        let mut candidates: Vec<u64> = match axis {
            TabDragAxis::Horizontal => self
                .tabs
                .iter()
                .filter(|tab| tab.pin.is_some() == pinned)
                .map(|tab| tab.id)
                .collect(),
            TabDragAxis::Vertical => self
                .listed_tabs(cx)
                .into_iter()
                .filter(|&index| self.tabs[index].pin.is_some() == pinned)
                .map(|index| self.tabs[index].id)
                .collect(),
        };
        candidates.retain(|&id| id != dragged_id);
        candidates.sort_by(|left, right| {
            let left = self.tab_slot_origin(*left, axis);
            let right = self.tab_slot_origin(*right, axis);
            left.partial_cmp(&right)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let coordinate = axis_coordinate(position, axis);
        let insertion = midpoint_insertion_index(
            candidates.iter().map(|id| {
                self.tab_geometry.get(id).map(|bounds| {
                    stable_slot_midpoint(
                        axis_origin(*bounds, axis),
                        axis_size(*bounds, axis),
                        self.tab_offset(*id, axis),
                    )
                })
            }),
            coordinate,
        );
        insert_id_at(&mut candidates, dragged_id, insertion);

        let current_order: Vec<u64> = match axis {
            TabDragAxis::Horizontal => self
                .tabs
                .iter()
                .filter(|tab| tab.pin.is_some() == pinned)
                .map(|tab| tab.id)
                .collect(),
            TabDragAxis::Vertical => self
                .listed_tabs(cx)
                .into_iter()
                .filter(|&index| self.tabs[index].pin.is_some() == pinned)
                .map(|index| self.tabs[index].id)
                .collect(),
        };
        let order_changed = current_order != candidates;
        if order_changed {
            self.move_tab_to_visible_order(dragged_id, &candidates);
        }

        let start = candidates
            .iter()
            .filter_map(|id| self.tab_slot_origin(*id, axis))
            .fold(f32::INFINITY, f32::min);
        if !start.is_finite() {
            return;
        }
        let gap = match axis {
            TabDragAxis::Horizontal => super::super::super::metrics::TAB_STRIP_GAP,
            TabDragAxis::Vertical => {
                super::super::super::metrics::SidebarSpacing::new(self.sidebar.shows_icons())
                    .item_gap
            }
        };
        let grab = self
            .tab_drag_layout
            .as_ref()
            .map(|layout| match axis {
                TabDragAxis::Horizontal => f32::from(layout.grab_offset.x),
                TabDragAxis::Vertical => f32::from(layout.grab_offset.y),
            })
            .unwrap_or(0.0);
        let mut target_position = start;
        let mut neighbor_animations = if order_changed {
            Vec::with_capacity(candidates.len())
        } else {
            Vec::new()
        };
        let mut dragged_offset = 0.0;
        for id in &candidates {
            let Some(bounds) = self.tab_geometry.get(id).copied() else {
                continue;
            };
            let current_visual = axis_origin(bounds, axis);
            let size = axis_size(bounds, axis);
            if *id == dragged_id {
                dragged_offset = coordinate - grab - target_position;
            } else if order_changed {
                neighbor_animations.push((*id, current_visual - target_position));
            }
            target_position += size + gap;
        }
        for (id, from) in neighbor_animations {
            match animation_retarget_offset(order_changed, from) {
                Some(from) => {
                    self.tab_position_animations.insert(
                        id,
                        super::super::TabPositionAnimation::new(
                            axis,
                            from,
                            Duration::from_millis(
                                super::super::super::metrics::TAB_REORDER_DURATION_MS,
                            ),
                            &Settings::motion(cx),
                        ),
                    );
                }
                None if order_changed => {
                    self.tab_position_animations.remove(&id);
                }
                None => {}
            }
        }
        if let Some(layout) = self.tab_drag_layout.as_mut() {
            layout.visual_offset = dragged_offset;
        }
        cx.notify();
    }

    fn move_tab_to_visible_order(&mut self, dragged_id: u64, order: &[u64]) {
        let Some(current_index) = self.tab_index_by_id(dragged_id) else {
            return;
        };
        let Some(position) = order.iter().position(|&id| id == dragged_id) else {
            return;
        };
        let before_id = order.get(position + 1).copied();
        let after_id = position
            .checked_sub(1)
            .and_then(|previous| order.get(previous).copied());
        let active_id = self.active_tab.map(|index| self.tabs[index].id);

        let tab = self.tabs.remove(current_index);
        let subscription = self.tab_subscriptions.remove(current_index);
        let focus_handle = self.tab_focus_handles.remove(current_index);
        let insertion_index = before_id
            .and_then(|id| self.tab_index_by_id(id))
            .or_else(|| {
                after_id
                    .and_then(|id| self.tab_index_by_id(id))
                    .map(|index| index + 1)
            })
            .unwrap_or_else(|| {
                if tab.pin.is_some() {
                    self.pinned_count()
                } else {
                    self.tabs.len()
                }
            });
        self.tabs.insert(insertion_index, tab);
        self.tab_subscriptions.insert(insertion_index, subscription);
        self.tab_focus_handles.insert(insertion_index, focus_handle);
        self.active_tab = active_id.and_then(|id| self.tab_index_by_id(id));
    }

    fn tab_offset(&self, id: u64, axis: TabDragAxis) -> f32 {
        if let Some(drag) = self.tab_drag_layout.as_ref().filter(|drag| drag.id == id) {
            return drag.visual_offset;
        }
        super::super::tab_position_animation_offset(self.tab_position_animations.get(&id), axis)
    }

    /// The tab's layout slot with its temporary drag animation removed. Using
    /// these stable slot coordinates keeps insertion thresholds from drifting
    /// as neighboring tabs animate around the pointer.
    fn tab_slot_origin(&self, id: u64, axis: TabDragAxis) -> Option<f32> {
        self.tab_geometry
            .get(&id)
            .copied()
            .map(|bounds| axis_origin(bounds, axis) - self.tab_offset(id, axis))
    }

    fn external_drop_target(
        &self,
        global_position: gpui::Point<gpui::Pixels>,
        previous_global_position: gpui::Point<gpui::Pixels>,
        current_window: gpui::AnyWindowHandle,
        source_bounds: gpui::Bounds<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) -> Option<(
        gpui::WindowHandle<BrowserWindow>,
        ExternalDropTargetKind,
        gpui::Point<gpui::Pixels>,
    )> {
        let mut nearest_window_target = None;
        let mut nearest_window_distance = f32::INFINITY;
        for handle in BrowserWindowRegistry::ordered_handles(cx) {
            if handle.window_id() == current_window.window_id() {
                // The source window blocks windows beneath it in the native
                // stack. The caller will resolve its local targets afterward.
                if source_bounds.contains(&global_position) {
                    return None;
                }
                continue;
            }
            let Some(target) = handle.downcast::<BrowserWindow>() else {
                continue;
            };
            let Ok((
                window_bounds,
                content_origin,
                viewport_size,
                favourites,
                tab_container,
                reveal_bounds,
            )) = target.update(cx, |browser, window, cx| {
                (
                    window.bounds(),
                    window.inner_window_bounds().get_bounds().origin,
                    window.viewport_size(),
                    browser.favourites_bounds,
                    browser.tab_container_bounds,
                    browser.favourites_drop_bounds(cx),
                )
            })
            else {
                continue;
            };
            // The destination's cached mouse position is stale while the source
            // owns pointer capture. Translate the live source pointer into the
            // destination's content coordinates instead.
            let local_position = content_local_position(global_position, content_origin);
            let viewport_bounds =
                gpui::Bounds::new(gpui::point(gpui::px(0.0), gpui::px(0.0)), viewport_size);
            let pointer_over_window = window_bounds.contains(&global_position);
            if !pointer_over_window {
                let distance = bounds_distance(window_bounds, global_position);
                if distance < nearest_window_distance
                    && is_over_or_moving_toward_bounds(
                        window_bounds,
                        global_position,
                        previous_global_position,
                    )
                {
                    nearest_window_distance = distance;
                    nearest_window_target =
                        Some((target, ExternalDropTargetKind::WindowBody, local_position));
                }
                continue;
            }
            match external_drop_target_kind(
                local_position,
                favourites,
                reveal_bounds,
                tab_container,
                viewport_bounds,
            ) {
                Some(ExternalDropTargetKind::Favourites) => {
                    return Some((target, ExternalDropTargetKind::Favourites, local_position));
                }
                Some(kind) => {
                    // The first window containing the pointer is the topmost
                    // app window reported by GPUI. Don't target a hidden strip.
                    return Some((target, kind, local_position));
                }
                None => {
                    return Some((target, ExternalDropTargetKind::WindowBody, local_position));
                }
            }
        }
        nearest_window_target
    }

    fn is_favourites_drop_position(&self, position: gpui::Point<gpui::Pixels>, cx: &App) -> bool {
        self.favourites_drop_bounds(cx)
            .is_some_and(|bounds| bounds.contains(&position))
    }

    /// Where a dropped tab becomes a favourite: the favourites and the gap
    /// around them, stopping short of the tab list so reordering the first
    /// tab still works.
    fn favourites_drop_bounds(&self, cx: &App) -> Option<gpui::Bounds<gpui::Pixels>> {
        let Some(favourites) = self.favourites_bounds else {
            return self.empty_favourites_reveal_bounds(cx);
        };
        let reach = gpui::px(super::super::super::metrics::SIDEBAR_FAVOURITES_DROP_REACH);
        let top = favourites.origin.y - reach;
        let mut bottom = favourites.bottom() + reach;
        if let Some(tabs) = self.tab_container_bounds
            && tabs.origin.y >= favourites.bottom()
        {
            bottom = bottom.min(tabs.origin.y);
        }
        Some(gpui::Bounds::from_corners(
            gpui::point(favourites.origin.x - reach, top),
            gpui::point(favourites.right() + reach, bottom),
        ))
    }

    /// When the empty-state hint was dismissed, keep a small, invisible
    /// target just above the tab list. Entering it reveals the real target
    /// without showing the hint for every tab drag.
    fn empty_favourites_reveal_bounds(&self, cx: &App) -> Option<gpui::Bounds<gpui::Pixels>> {
        if self.favourites_bounds.is_some() {
            return None;
        }
        let layout = WorkspaceLayout::current(cx);
        let has_favourites_panel = layout.sidebar_region().is_some_and(|region| {
            layout.components(region).any(|component| {
                component.id == super::super::super::workspace_layout::ComponentId::FavouritesPanel
            })
        });
        if !has_favourites_panel
            || self.tab_axis(cx) != TabDragAxis::Vertical
            || self.sidebar.shows_icons()
        {
            return None;
        }
        let space = Settings::get(cx).spaces.active();
        if !space.favourites.is_empty() || !space.favourites_hint_dismissed {
            return None;
        }
        let tabs = self.tab_container_bounds?;
        let height = gpui::px(
            2.0 * super::super::super::metrics::SIDEBAR_SEPARATOR_MARGIN
                + super::super::super::metrics::MENU_SEPARATOR_HEIGHT,
        );
        Some(gpui::Bounds::new(
            gpui::point(tabs.origin.x, tabs.origin.y - height),
            gpui::size(tabs.size.width, height),
        ))
    }

    fn set_favourites_drag_hover(
        &mut self,
        source: gpui::AnyWindowHandle,
        id: u64,
        cx: &mut Context<Self>,
    ) {
        let target = Some((source, id));
        if self.favourites_drag_hover != target {
            self.favourites_drag_hover = target;
            cx.notify();
        }
    }

    pub(super) fn clear_favourites_drag_hover(
        &mut self,
        source: gpui::AnyWindowHandle,
        id: u64,
        cx: &mut Context<Self>,
    ) {
        if self.favourites_drag_hover == Some((source, id)) {
            self.favourites_drag_hover = None;
            cx.notify();
        }
    }

    fn set_tab_drag_phase(&mut self, phase: super::super::TabDragPhase, cx: &mut Context<Self>) {
        let changed = self.tab_drag_layout.as_mut().is_some_and(|layout| {
            if layout.phase == phase {
                false
            } else {
                layout.phase = phase;
                true
            }
        });
        if changed {
            cx.notify();
        }
    }

    pub(super) fn notify_other_tab_drop_targets(
        cx: &mut App,
        current_window: gpui::AnyWindowHandle,
    ) {
        for handle in cx.windows() {
            if handle.window_id() == current_window.window_id() {
                continue;
            }
            let Some(browser) = handle.downcast::<BrowserWindow>() else {
                continue;
            };
            browser.update(cx, |_, _, cx| cx.notify()).ok();
        }
    }

    fn set_cross_window_target(
        &mut self,
        target: gpui::WindowHandle<BrowserWindow>,
        dragged: &DraggedTab,
        position: gpui::Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) {
        let target_handle = target.into();
        let previous = self
            .tab_drag_layout
            .as_ref()
            .and_then(|layout| layout.target_window);
        if previous != Some(target_handle) {
            self.clear_previous_target(cx);
        }
        if target
            .update(cx, |browser, _, cx| {
                browser.clear_favourites_drag_hover(dragged.source, dragged.id, cx);
                browser.set_incoming_tab_drag(dragged, position, cx);
            })
            .is_err()
        {
            return;
        }
        if let Some(layout) = self.tab_drag_layout.as_mut() {
            layout.phase = super::super::TabDragPhase::CrossWindowTargeting;
            layout.target_window = Some(target_handle);
        }
    }

    fn update_incoming_drag(
        &mut self,
        dragged: &DraggedTab,
        position: gpui::Point<gpui::Pixels>,
        current_window: gpui::AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if self.is_favourites_drop_position(position, cx) {
            self.clear_incoming_tab_drag(dragged.source, dragged.id, cx);
            self.set_favourites_drag_hover(dragged.source, dragged.id, cx);
            self.set_source_drag_phase(
                dragged.source,
                dragged.id,
                super::super::TabDragPhase::DraggingOverFavourites,
                None,
                cx,
            );
            return;
        }
        self.clear_favourites_drag_hover(dragged.source, dragged.id, cx);
        if self
            .tab_container_bounds
            .is_some_and(|bounds| bounds.contains(&position))
        {
            self.set_incoming_tab_drag(dragged, position, cx);
            self.set_source_drag_phase(
                dragged.source,
                dragged.id,
                super::super::TabDragPhase::CrossWindowTargeting,
                Some(current_window),
                cx,
            );
            return;
        }
        self.clear_incoming_tab_drag(dragged.source, dragged.id, cx);
        self.set_source_drag_phase(
            dragged.source,
            dragged.id,
            super::super::TabDragPhase::OutsideTarget,
            None,
            cx,
        );
    }

    fn set_source_drag_phase(
        &self,
        source: gpui::AnyWindowHandle,
        id: u64,
        phase: super::super::TabDragPhase,
        target: Option<gpui::AnyWindowHandle>,
        cx: &mut App,
    ) {
        let Some(source) = source.downcast::<BrowserWindow>() else {
            return;
        };
        source
            .update(cx, |browser, _, _| {
                if let Some(drag) = browser
                    .tab_drag_layout
                    .as_mut()
                    .filter(|drag| drag.id == id)
                {
                    drag.phase = phase;
                    drag.target_window = target;
                }
            })
            .ok();
    }

    pub(super) fn clear_previous_target(&mut self, cx: &mut App) {
        let Some((target, source, id)) = self.tab_drag_layout.as_mut().and_then(|layout| {
            layout
                .target_window
                .take()
                .map(|target| (target, layout.source_window, layout.id))
        }) else {
            return;
        };
        let Some(target) = target.downcast::<BrowserWindow>() else {
            return;
        };
        target
            .update(cx, |browser, _, cx| {
                browser.clear_incoming_tab_drag(source, id, cx);
                browser.clear_favourites_drag_hover(source, id, cx);
            })
            .ok();
    }

    fn set_incoming_tab_drag(
        &mut self,
        dragged: &DraggedTab,
        position: gpui::Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) {
        let axis = self.tab_axis(cx);
        let (before_id, after_id) = self.insertion_neighbors(dragged.pinned, axis, position, cx);
        let unchanged = self.incoming_tab_drag.as_ref().is_some_and(|incoming| {
            incoming.source == dragged.source
                && incoming.id == dragged.id
                && incoming.axis == axis
                && incoming.before_id == before_id
                && incoming.after_id == after_id
        });
        if unchanged {
            if let Some(incoming) = self.incoming_tab_drag.as_mut() {
                incoming.pointer_position = position;
            }
            return;
        }
        let incoming = super::super::IncomingTabDrag {
            source: dragged.source,
            id: dragged.id,
            axis,
            pointer_position: position,
            before_id,
            after_id,
            size: incoming_marker_size(axis, dragged.size, dragged.pinned),
            pinned: dragged.pinned,
        };
        self.animate_incoming_layout(&incoming, true, cx);
        self.incoming_tab_drag = Some(incoming);
        cx.notify();
    }

    fn clear_incoming_tab_drag(
        &mut self,
        source: gpui::AnyWindowHandle,
        id: u64,
        cx: &mut Context<Self>,
    ) {
        if !self
            .incoming_tab_drag
            .as_ref()
            .is_some_and(|incoming| incoming.source == source && incoming.id == id)
        {
            return;
        }
        let Some(incoming) = self.incoming_tab_drag.clone() else {
            return;
        };
        self.animate_incoming_layout(&incoming, false, cx);
        self.incoming_tab_drag = None;
        cx.notify();
    }

    fn insertion_neighbors(
        &self,
        pinned: bool,
        axis: TabDragAxis,
        position: gpui::Point<gpui::Pixels>,
        cx: &App,
    ) -> (Option<u64>, Option<u64>) {
        let mut candidates: Vec<u64> = match axis {
            TabDragAxis::Horizontal => self.tabs.iter().map(|tab| tab.id).collect(),
            TabDragAxis::Vertical => self
                .listed_tabs(cx)
                .into_iter()
                .map(|index| self.tabs[index].id)
                .collect(),
        };
        candidates.retain(|id| {
            self.tab_index_by_id(*id)
                .is_some_and(|index| self.tabs[index].pin.is_some() == pinned)
        });
        candidates.sort_by(|left, right| {
            let left = self.tab_slot_origin(*left, axis);
            let right = self.tab_slot_origin(*right, axis);
            left.partial_cmp(&right)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let coordinate = axis_coordinate(position, axis);
        let insertion = midpoint_insertion_index(
            candidates.iter().map(|id| {
                self.tab_geometry.get(id).map(|bounds| {
                    stable_slot_midpoint(
                        axis_origin(*bounds, axis),
                        axis_size(*bounds, axis),
                        self.tab_offset(*id, axis),
                    )
                })
            }),
            coordinate,
        );
        (
            candidates.get(insertion).copied(),
            insertion
                .checked_sub(1)
                .and_then(|index| candidates.get(index).copied()),
        )
    }

    fn animate_incoming_layout(
        &mut self,
        incoming: &super::super::IncomingTabDrag,
        include_marker: bool,
        cx: &App,
    ) {
        let axis = incoming.axis;
        let mut ids: Vec<u64> = match axis {
            TabDragAxis::Horizontal => self.tabs.iter().map(|tab| tab.id).collect(),
            TabDragAxis::Vertical => self
                .listed_tabs(cx)
                .into_iter()
                .map(|index| self.tabs[index].id)
                .collect(),
        };
        ids.retain(|id| {
            self.tab_index_by_id(*id)
                .is_some_and(|index| self.tabs[index].pin.is_some() == incoming.pinned)
        });
        if ids.is_empty() {
            return;
        }
        let mut start = ids
            .iter()
            .filter_map(|id| {
                self.tab_geometry
                    .get(id)
                    .copied()
                    .map(|bounds| axis_origin(bounds, axis) - self.tab_offset(*id, axis))
            })
            .fold(f32::INFINITY, f32::min);
        if !start.is_finite() {
            return;
        }
        let gap = match axis {
            TabDragAxis::Horizontal => super::super::super::metrics::TAB_STRIP_GAP,
            TabDragAxis::Vertical => {
                super::super::super::metrics::SidebarSpacing::new(self.sidebar.shows_icons())
                    .item_gap
            }
        };
        let leading_gap = self
            .incoming_tab_drag
            .as_ref()
            .filter(|previous| previous.before_id == ids.first().copied())
            .map_or(0.0, |previous| previous.size + gap);
        start -= leading_gap;
        let mut cursor = start;
        let mut animations = Vec::with_capacity(ids.len());
        for id in ids {
            if include_marker && incoming.before_id == Some(id) {
                cursor += incoming.size + gap;
            }
            let Some(bounds) = self.tab_geometry.get(&id).copied() else {
                continue;
            };
            animations.push((id, axis_origin(bounds, axis) - cursor));
            cursor += axis_size(bounds, axis) + gap;
            if include_marker && incoming.after_id == Some(id) {
                cursor += incoming.size + gap;
            }
        }
        for (id, from) in animations {
            if from.abs() > 0.1 {
                self.tab_position_animations.insert(
                    id,
                    super::super::TabPositionAnimation::new(
                        axis,
                        from,
                        Duration::from_millis(
                            super::super::super::metrics::TAB_REORDER_DURATION_MS,
                        ),
                        &Settings::motion(cx),
                    ),
                );
            } else {
                self.tab_position_animations.remove(&id);
            }
        }
    }

    pub(in super::super::super) fn finish_tab_drag(
        &mut self,
        release_position: gpui::Point<gpui::Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(dragged) = cx
            .active_drag()
            .and_then(|drag| drag.value.downcast_ref::<DraggedTab>())
            .filter(|dragged| dragged.source != window.window_handle())
            .cloned()
        {
            self.clear_incoming_tab_drag(dragged.source, dragged.id, cx);
            self.clear_favourites_drag_hover(dragged.source, dragged.id, cx);
            if let Some(source) = dragged.source.downcast::<BrowserWindow>() {
                source
                    .update(cx, |browser, _, cx| {
                        if browser
                            .tab_drag_layout
                            .as_ref()
                            .is_some_and(|layout| layout.id == dragged.id)
                        {
                            browser.tab_drag_layout = None;
                            browser.suppress_tab_click_until =
                                Some(Instant::now() + Duration::from_millis(250));
                            cx.notify();
                        }
                    })
                    .ok();
            }
            Self::notify_other_tab_drop_targets(cx, window.window_handle());
            return;
        }

        // Released over the favourites' reach, which can lie outside the
        // drawn tiles that receive drops directly.
        let favourite_drop = self.tab_drag_layout.as_ref().and_then(|drag| {
            (drag.source_window == window.window_handle()
                && drag.phase == super::super::TabDragPhase::DraggingOverFavourites
                && drag.target_window.is_none()
                && self.is_favourites_drop_position(release_position, cx))
            .then_some(drag.id)
        });
        if let Some(id) = favourite_drop {
            self.favourite_dropped_tab(id, cx);
        }

        let release_target = self.tab_drag_layout.as_ref().and_then(|drag| {
            (drag.source_window == window.window_handle()
                && matches!(
                    drag.phase,
                    super::super::TabDragPhase::Reordering
                        | super::super::TabDragPhase::OutsideTarget
                )
                && self
                    .tab_container_bounds
                    .is_some_and(|bounds| bounds.contains(&release_position))
                && !self
                    .favourites_drop_bounds(cx)
                    .is_some_and(|bounds| bounds.contains(&release_position)))
            .then_some((drag.id, drag.axis, drag.pinned, drag.measured_reorder))
        });
        if let Some((id, axis, pinned, measured_reorder)) = release_target {
            if measured_reorder {
                let order = self.tab_reorder_groups.group(axis, pinned).order().to_vec();
                self.apply_tab_reorder(axis, pinned, &order, cx);
            } else {
                self.reorder_tab_at_pointer(id, axis, release_position, cx, window);
            }
        }

        self.clear_previous_target(cx);
        let Some(mut drag) = self.tab_drag_layout.take() else {
            return;
        };
        self.clear_favourites_drag_hover(drag.source_window, drag.id, cx);
        if drag.phase == super::super::TabDragPhase::PendingDrag {
            return;
        }
        drag.phase = super::super::TabDragPhase::Completing;
        let current_offset = drag.visual_offset;
        if !drag.measured_reorder && current_offset.abs() > 0.1 {
            self.tab_position_animations.insert(
                drag.id,
                super::super::TabPositionAnimation::new(
                    drag.axis,
                    current_offset,
                    Duration::from_millis(
                        super::super::super::metrics::TAB_DROP_SETTLE_DURATION_MS,
                    ),
                    &Settings::motion(cx),
                ),
            );
        } else {
            self.tab_position_animations.remove(&drag.id);
        }
        self.suppress_tab_click_until = Some(Instant::now() + Duration::from_millis(250));
        Self::notify_other_tab_drop_targets(cx, window.window_handle());
        cx.notify();
        window.refresh();
    }

    pub(in super::super::super) fn favourite_drop_handler(&self, cx: &Context<Self>) -> TabDrop {
        let browser = cx.entity().downgrade();
        Rc::new(move |dragged, window, cx| {
            if dragged.source == window.window_handle() {
                browser
                    .update(cx, |this, cx| this.add_dragged_favourite(dragged.id, cx))
                    .ok();
            } else if let Some(source) = dragged.source.downcast::<BrowserWindow>() {
                source
                    .update(cx, |this, _, cx| this.add_dragged_favourite(dragged.id, cx))
                    .ok();
            }
        })
    }

    pub(in super::super::super) fn favourite_bounds_handler(
        &self,
        cx: &Context<Self>,
    ) -> TabPrepaint {
        let browser = cx.entity().downgrade();
        Box::new(move |bounds, _, cx| {
            browser
                .update(cx, |this, _| this.favourites_bounds = Some(bounds))
                .ok();
        })
    }

    pub(in super::super::super) fn tab_container_bounds_handler(
        &self,
        cx: &Context<Self>,
    ) -> TabPrepaint {
        let browser = cx.entity().downgrade();
        Box::new(move |bounds, window, cx| {
            let animate_button = browser
                .update(cx, |this, cx| {
                    this.tab_container_bounds = Some(bounds);
                    this.auto_scroll_tab_strip(window, cx);
                    let running = this
                        .new_tab_button_animation
                        .as_ref()
                        .is_some_and(|animation| animation.position.is_running());
                    if !running {
                        this.new_tab_button_animation = None;
                    }
                    running
                })
                .unwrap_or(false);
            if animate_button {
                window.request_animation_frame();
            }
        })
    }

    pub(in super::super::super) fn new_tab_button_motion(&self) -> Option<(String, f32, Duration)> {
        self.new_tab_button_animation
            .as_ref()
            .map(|animation| (animation.id.clone(), animation.from, animation.duration))
    }

    pub(super) fn new_tab_button_position_offset(&self) -> f32 {
        self.new_tab_button_animation
            .as_ref()
            .map_or(0.0, |animation| animation.position.value())
    }

    pub(super) fn start_new_tab_button_animation(
        &mut self,
        from: f32,
        duration: Duration,
        settings: &super::super::super::motion::MotionConfig,
    ) {
        let id = NEXT_TAB_BUTTON_MOTION_ID.fetch_add(1, Ordering::Relaxed);
        self.new_tab_button_animation = Some(super::super::NewTabButtonAnimation {
            id: format!("browser-new-tab-position-{id}"),
            from,
            duration,
            position: super::super::TabPositionAnimation::with_curve(
                TabDragAxis::Vertical,
                from,
                duration,
                settings,
                super::super::super::motion::TweenCurve::EaseOut,
            ),
        });
    }

    fn auto_scroll_tab_strip(&mut self, window: &mut Window, cx: &Context<Self>) {
        if self.tab_axis(cx) != TabDragAxis::Horizontal {
            self.tab_auto_scroll_at = None;
            return;
        }
        let pointer = self
            .tab_drag_layout
            .as_ref()
            .filter(|drag| {
                drag.source_window == window.window_handle()
                    && !matches!(
                        drag.phase,
                        super::super::TabDragPhase::PendingDrag
                            | super::super::TabDragPhase::Completing
                            | super::super::TabDragPhase::Cancelled
                    )
            })
            .map(|drag| drag.pointer_position)
            .or_else(|| {
                self.incoming_tab_drag
                    .as_ref()
                    .filter(|incoming| incoming.axis == TabDragAxis::Horizontal)
                    .map(|incoming| incoming.pointer_position)
            });
        let Some((bounds, pointer)) = self.tab_container_bounds.zip(pointer) else {
            self.tab_auto_scroll_at = None;
            return;
        };

        let left = f32::from(bounds.left());
        let right = f32::from(bounds.right());
        let x = f32::from(pointer.x);
        if !(left..=right).contains(&x) {
            self.tab_auto_scroll_at = None;
            return;
        }

        let edge = super::super::super::metrics::TAB_DRAG_SCROLL_EDGE;
        let left_distance = x - left;
        let right_distance = right - x;
        let (direction, proximity) = if left_distance < edge && left_distance <= right_distance {
            (1.0, (1.0 - left_distance / edge).clamp(0.0, 1.0))
        } else if right_distance < edge {
            (-1.0, (1.0 - right_distance / edge).clamp(0.0, 1.0))
        } else {
            self.tab_auto_scroll_at = None;
            return;
        };

        let now = Instant::now();
        let elapsed = self
            .tab_auto_scroll_at
            .replace(now)
            .map_or(1.0 / 60.0, |last| {
                now.duration_since(last).as_secs_f32().min(1.0 / 30.0)
            });
        let offset = self.tab_strip_scroll.offset();
        let max_offset = f32::from(self.tab_strip_scroll.max_offset().x);
        let current = f32::from(offset.x);
        let next = (current
            + direction
                * super::super::super::metrics::TAB_DRAG_SCROLL_SPEED
                * proximity
                * elapsed)
            .clamp(-max_offset, 0.0);
        if (next - current).abs() <= 0.01 {
            self.tab_auto_scroll_at = None;
            return;
        }

        self.tab_strip_scroll
            .set_offset(gpui::point(gpui::px(next), offset.y));
        window.request_animation_frame();
    }

    pub(in super::super::super) fn tab_target_drop_handler(
        &self,
        target_id: u64,
        cx: &Context<Self>,
    ) -> TabDrop {
        let target = cx.entity().downgrade();
        let transfer = self.tab_drop_handler(cx);
        Rc::new(move |dragged, window, cx| {
            if dragged.source != window.window_handle() {
                let Some(target_pinned) = target
                    .update(cx, |browser, _| {
                        browser
                            .tab_index_by_id(target_id)
                            .map(|index| browser.tabs[index].pin.is_some())
                    })
                    .ok()
                    .flatten()
                else {
                    return;
                };
                let Some(source) = dragged.source.downcast::<BrowserWindow>() else {
                    return;
                };
                let can_transfer = source
                    .update(cx, |browser, _, cx| {
                        let Some(index) = browser.tab_index_by_id(dragged.id) else {
                            return false;
                        };
                        if browser.tabs[index].pin.is_some() != target_pinned {
                            if target_pinned && !browser.can_pin(index, cx) {
                                return false;
                            }
                            browser.toggle_pin(index, cx);
                        }
                        true
                    })
                    .unwrap_or(false);
                if !can_transfer {
                    return;
                }
                transfer(dragged, window, cx);
                return;
            }
            target
                .update(cx, |browser, cx| {
                    let Some(from) = browser.tab_index_by_id(dragged.id) else {
                        return;
                    };
                    let Some(to) = browser.tab_index_by_id(target_id) else {
                        return;
                    };
                    if browser.tabs[from].pin.is_some() != browser.tabs[to].pin.is_some() {
                        browser.drop_tab(from, to, cx);
                    }
                })
                .ok();
        })
    }

    pub(in super::super::super) fn drop_tab_into_today_handler(
        &self,
        cx: &Context<Self>,
    ) -> TabDrop {
        let transfer = self.tab_drop_handler(cx);
        Rc::new(move |dragged, window, cx| {
            if let Some(source) = dragged.source.downcast::<BrowserWindow>() {
                source
                    .update(cx, |browser, _, cx| {
                        if let Some(index) = browser.tab_index_by_id(dragged.id) {
                            browser.drop_tab_into_today(index, cx);
                        }
                    })
                    .ok();
            }
            if dragged.source != window.window_handle() {
                transfer(dragged, window, cx);
            }
        })
    }

    pub(in super::super::super) fn tab_drop_handler(&self, cx: &Context<Self>) -> TabDrop {
        let target = cx.entity().downgrade();
        Rc::new(move |dragged, window, cx| {
            if dragged.source == window.window_handle() {
                return;
            }
            let Some(source) = dragged.source.downcast::<BrowserWindow>() else {
                return;
            };
            let Ok(Some((source_tab, source_ids))) = source.read(cx).map(|browser| {
                browser.tab_index_by_id(dragged.id).and_then(|index| {
                    browser.tabs.get(index).cloned().map(|tab| {
                        (
                            tab,
                            browser.tabs.iter().map(|tab| tab.id).collect::<Vec<_>>(),
                        )
                    })
                })
            }) else {
                return;
            };
            let target_position = window.mouse_position();
            let inserted_id = target
                .update(cx, |browser, cx| {
                    let axis = browser.tab_axis(cx);
                    let pinned = source_tab.pin.is_some();
                    let index = browser.insertion_index_for_tab(pinned, axis, target_position, cx);
                    let target_ids: Vec<_> = browser.tabs.iter().map(|tab| tab.id).collect();
                    let desired_id = if target_ids.contains(&dragged.id) {
                        BrowserWindow::allocate_tab_id()
                    } else {
                        dragged.id
                    };
                    let plan =
                        plan_tab_transfer(&source_ids, &target_ids, dragged.id, desired_id, index)?;
                    browser.insert_tab_at_immediately(
                        plan.target_index,
                        source_tab.content.clone(),
                        source_tab.pin.clone(),
                        true,
                        window,
                        cx,
                    );
                    let inserted = browser.tabs[plan.target_index].id;
                    if let Some(inserted_index) = browser.tab_index_by_id(inserted) {
                        browser.tabs[inserted_index].id = plan.target_id;
                        browser.tabs[inserted_index].space = source_tab.space;
                        browser.tabs[inserted_index].favourite = source_tab.favourite.clone();
                    }
                    if browser.incoming_tab_drag.as_ref().is_some_and(|incoming| {
                        incoming.source == dragged.source && incoming.id == dragged.id
                    }) {
                        browser.incoming_tab_drag = None;
                    }
                    Some(plan)
                })
                .ok();
            let Some(Some(plan)) = inserted_id else {
                return;
            };
            source
                .update(cx, |browser, source_window, cx| {
                    let index = browser
                        .tabs
                        .get(plan.source_index)
                        .filter(|tab| tab.id == dragged.id)
                        .map(|_| plan.source_index)
                        .or_else(|| browser.tab_index_by_id(dragged.id));
                    if let Some(index) = index {
                        browser.tab_drag_layout = None;
                        browser.remove_tab_for_transfer(index, source_window, cx);
                    }
                })
                .ok();
            target
                .update(cx, |browser, cx| {
                    if let Some(index) = browser.tab_index_by_id(plan.target_id) {
                        browser.activate_tab(index, true, window, cx);
                    }
                })
                .ok();
        })
    }

    fn add_dragged_favourite(&mut self, id: u64, cx: &mut Context<Self>) {
        if self
            .favourites_drag_hover
            .is_some_and(|(_, hovered_id)| hovered_id == id)
        {
            self.favourites_drag_hover = None;
            cx.notify();
        }
        self.clear_previous_target(cx);
        self.tab_drag_layout = None;
        self.favourite_dropped_tab(id, cx);
    }

    /// Makes the dragged tab a favourite, unless it cannot be one or already
    /// is.
    fn favourite_dropped_tab(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(index) = self.tab_index_by_id(id) else {
            return;
        };
        if self.tabs[index].favourite.is_some()
            || matches!(&self.tabs[index].content, TabContent::Page(_))
        {
            return;
        }
        let Some(shortcut) = super::super::pinned::pin_for(&self.tabs[index], cx) else {
            return;
        };
        self.add_favourite(index, shortcut, cx);
    }

    fn insertion_index_for_tab(
        &self,
        pinned: bool,
        axis: TabDragAxis,
        position: gpui::Point<gpui::Pixels>,
        cx: &App,
    ) -> usize {
        let mut candidates: Vec<usize> = match axis {
            TabDragAxis::Horizontal => (0..self.tabs.len()).collect(),
            TabDragAxis::Vertical => self.listed_tabs(cx),
        }
        .into_iter()
        .filter(|&index| self.tabs[index].pin.is_some() == pinned)
        .collect();
        candidates.sort_by(|left, right| {
            let left = self.tab_slot_origin(self.tabs[*left].id, axis);
            let right = self.tab_slot_origin(self.tabs[*right].id, axis);
            left.partial_cmp(&right)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let coordinate = axis_coordinate(position, axis);
        let insertion = midpoint_insertion_index(
            candidates.iter().map(|&index| {
                self.tab_geometry.get(&self.tabs[index].id).map(|bounds| {
                    stable_slot_midpoint(
                        axis_origin(*bounds, axis),
                        axis_size(*bounds, axis),
                        self.tab_offset(self.tabs[index].id, axis),
                    )
                })
            }),
            coordinate,
        );
        if let Some(index) = candidates.get(insertion) {
            return *index;
        }
        candidates.last().map_or_else(
            || {
                if pinned {
                    self.pinned_count()
                } else {
                    self.tabs.len()
                }
            },
            |last| last + 1,
        )
    }

    fn remove_tab_for_transfer(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.tabs.len() {
            return;
        }
        let was_active = self.active_tab == Some(index);
        let next = was_active
            .then(|| self.tab_after_closing(index, cx))
            .flatten();
        if let Some(webview) = self.tabs[index].content.webview() {
            webview.update(cx, |view, _| {
                view.set_page_visible(false);
                view.session.set_focus(false);
            });
        }
        self.revealed_icons
            .borrow_mut()
            .remove(&self.tabs[index].id);
        self.tab_entry_animations
            .borrow_mut()
            .remove(&self.tabs[index].id);
        self.tab_position_animations.remove(&self.tabs[index].id);
        self.tab_geometry.remove(&self.tabs[index].id);
        self.tabs.remove(index);
        drop(self.tab_subscriptions.remove(index));
        self.tab_focus_handles.remove(index);
        if was_active {
            self.active_tab = next;
        } else if self.active_tab.is_some_and(|active| index < active) {
            self.active_tab = self.active_tab.map(|active| active - 1);
        }

        if self.tabs.is_empty() {
            self.active_tab = None;
            self.close_find_bar(false, window, cx);
            self.omnibox.update(cx, |omnibox, cx| {
                omnibox.set_content(None, None, window, cx)
            });
            if self.tab_axis(cx) == TabDragAxis::Horizontal
                && !Self::closes_with_last_tab(window, cx)
            {
                self.open_tab(window, cx);
            } else {
                self.record_session_state(WindowSession::default(), cx);
                window.remove_window();
            }
            return;
        } else if was_active && let Some(next) = self.active_tab {
            self.activate_tab(next, true, window, cx);
        }
        self.wake_tab_sleep_scheduler();
        cx.notify();
    }
}

fn axis_coordinate(point: gpui::Point<gpui::Pixels>, axis: TabDragAxis) -> f32 {
    match axis {
        TabDragAxis::Horizontal => f32::from(point.x),
        TabDragAxis::Vertical => f32::from(point.y),
    }
}

fn axis_origin(bounds: gpui::Bounds<gpui::Pixels>, axis: TabDragAxis) -> f32 {
    match axis {
        TabDragAxis::Horizontal => f32::from(bounds.origin.x),
        TabDragAxis::Vertical => f32::from(bounds.origin.y),
    }
}

fn axis_size(bounds: gpui::Bounds<gpui::Pixels>, axis: TabDragAxis) -> f32 {
    match axis {
        TabDragAxis::Horizontal => f32::from(bounds.size.width),
        TabDragAxis::Vertical => f32::from(bounds.size.height),
    }
}

fn bounds_distance(bounds: gpui::Bounds<gpui::Pixels>, point: gpui::Point<gpui::Pixels>) -> f32 {
    let left = f32::from(bounds.origin.x);
    let top = f32::from(bounds.origin.y);
    let dx = (left - f32::from(point.x))
        .max(f32::from(point.x) - left - f32::from(bounds.size.width))
        .max(0.0);
    let dy = (top - f32::from(point.y))
        .max(f32::from(point.y) - top - f32::from(bounds.size.height))
        .max(0.0);
    dx.hypot(dy)
}

fn is_over_or_moving_toward_bounds(
    bounds: gpui::Bounds<gpui::Pixels>,
    position: gpui::Point<gpui::Pixels>,
    previous_position: gpui::Point<gpui::Pixels>,
) -> bool {
    bounds.contains(&position)
        || bounds_distance(bounds, position) < bounds_distance(bounds, previous_position)
}

fn content_local_position(
    global_position: gpui::Point<gpui::Pixels>,
    content_origin: gpui::Point<gpui::Pixels>,
) -> gpui::Point<gpui::Pixels> {
    global_position - content_origin
}

fn external_drop_target_kind(
    position: gpui::Point<gpui::Pixels>,
    favourites: Option<gpui::Bounds<gpui::Pixels>>,
    reveal: Option<gpui::Bounds<gpui::Pixels>>,
    tab_strip: Option<gpui::Bounds<gpui::Pixels>>,
    viewport: gpui::Bounds<gpui::Pixels>,
) -> Option<ExternalDropTargetKind> {
    if favourites.is_some_and(|bounds| bounds.contains(&position))
        || reveal.is_some_and(|bounds| bounds.contains(&position))
    {
        Some(ExternalDropTargetKind::Favourites)
    } else if tab_strip.is_some_and(|bounds| bounds.contains(&position)) {
        Some(ExternalDropTargetKind::TabStrip)
    } else if viewport.contains(&position) {
        Some(ExternalDropTargetKind::WindowBody)
    } else {
        None
    }
}

fn incoming_marker_size(axis: TabDragAxis, size: f32, pinned: bool) -> f32 {
    match axis {
        TabDragAxis::Horizontal if pinned => {
            size.max(super::super::super::metrics::PINNED_TAB_WIDTH)
        }
        TabDragAxis::Horizontal => size.max(super::super::super::metrics::TAB_MIN_WIDTH),
        TabDragAxis::Vertical => super::super::super::metrics::SIDEBAR_TAB_HEIGHT,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TabTransferPlan {
    source_index: usize,
    target_index: usize,
    target_id: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExternalDropTargetKind {
    Favourites,
    TabStrip,
    WindowBody,
}

fn midpoint_insertion_index(
    midpoints: impl IntoIterator<Item = Option<f32>>,
    coordinate: f32,
) -> usize {
    let mut count = 0;
    for (index, midpoint) in midpoints.into_iter().enumerate() {
        count = index + 1;
        if midpoint.is_some_and(|midpoint| coordinate < midpoint) {
            return index;
        }
    }
    count
}

fn stable_slot_midpoint(visual_origin: f32, extent: f32, visual_offset: f32) -> f32 {
    visual_origin - visual_offset + extent / 2.0
}

fn animation_retarget_offset(order_changed: bool, current_offset: f32) -> Option<f32> {
    (order_changed && current_offset.abs() > 0.1).then_some(current_offset)
}

fn insert_id_at(order: &mut Vec<u64>, id: u64, index: usize) {
    order.retain(|candidate| *candidate != id);
    order.insert(index.min(order.len()), id);
}

fn plan_tab_transfer(
    source_ids: &[u64],
    target_ids: &[u64],
    moving_id: u64,
    target_id: u64,
    target_index: usize,
) -> Option<TabTransferPlan> {
    let mut source_matches = source_ids
        .iter()
        .enumerate()
        .filter(|(_, id)| **id == moving_id);
    let source_index = source_matches.next()?.0;
    if source_matches.next().is_some() || target_ids.contains(&target_id) {
        return None;
    }
    Some(TabTransferPlan {
        source_index,
        target_index: target_index.min(target_ids.len()),
        target_id,
    })
}

#[cfg(test)]
mod tab_drag_tests {
    use super::super::super::{TabPositionAnimation, tab_position_animation_offset};
    use super::super::tab_navigation_offset;
    use super::TabDragAxis;
    use super::{
        ExternalDropTargetKind, animation_retarget_offset, external_drop_target_kind, insert_id_at,
        is_over_or_moving_toward_bounds, midpoint_insertion_index, plan_tab_transfer,
        stable_slot_midpoint,
    };

    #[test]
    fn tab_navigation_uses_the_layout_axis() {
        assert_eq!(
            tab_navigation_offset("left", TabDragAxis::Horizontal),
            Some(-1)
        );
        assert_eq!(
            tab_navigation_offset("right", TabDragAxis::Horizontal),
            Some(1)
        );
        assert_eq!(tab_navigation_offset("up", TabDragAxis::Vertical), Some(-1));
        assert_eq!(
            tab_navigation_offset("down", TabDragAxis::Vertical),
            Some(1)
        );
        assert_eq!(tab_navigation_offset("up", TabDragAxis::Horizontal), None);
        assert_eq!(tab_navigation_offset("left", TabDragAxis::Vertical), None);
    }

    #[test]
    fn midpoint_insertion_tracks_both_directions_and_edges() {
        let midpoints = [Some(50.0), Some(120.0), Some(190.0)];
        assert_eq!(midpoint_insertion_index(midpoints, 49.0), 0);
        assert_eq!(midpoint_insertion_index(midpoints, 120.0), 2);
        assert_eq!(midpoint_insertion_index(midpoints, 250.0), 3);
    }

    #[test]
    fn reorder_keeps_stable_ids_and_can_reverse_direction() {
        let mut order = vec![10, 20, 30, 40];
        insert_id_at(&mut order, 10, 3);
        assert_eq!(order, [20, 30, 40, 10]);
        insert_id_at(&mut order, 10, 0);
        assert_eq!(order, [10, 20, 30, 40]);
    }

    #[test]
    fn insertion_grid_stays_fixed_while_tabs_animate() {
        let horizontal_midpoint = stable_slot_midpoint(140.0, 80.0, 40.0);
        let horizontal_midpoint_mid_animation = stable_slot_midpoint(119.0, 80.0, 19.0);
        let vertical_midpoint = stable_slot_midpoint(92.0, 36.0, 12.0);

        assert_eq!(horizontal_midpoint, 140.0);
        assert_eq!(horizontal_midpoint_mid_animation, horizontal_midpoint);
        assert_eq!(vertical_midpoint, 98.0);
    }

    #[test]
    fn neighbor_animation_only_retargets_after_the_slot_changes() {
        assert_eq!(animation_retarget_offset(false, 24.0), None);
        assert_eq!(animation_retarget_offset(true, 24.0), Some(24.0));
        assert_eq!(animation_retarget_offset(true, 0.05), None);
    }

    #[test]
    fn old_orientation_offsets_do_not_leak_into_the_new_layout() {
        let animation = TabPositionAnimation {
            axis: TabDragAxis::Horizontal,
            tween: super::super::super::Tween::at(24.0),
        };
        assert!(tab_position_animation_offset(Some(&animation), TabDragAxis::Horizontal) > 0.0);
        assert_eq!(
            tab_position_animation_offset(Some(&animation), TabDragAxis::Vertical),
            0.0
        );
    }

    #[test]
    fn transfer_plan_preserves_identity_and_rejects_duplicates() {
        let plan = plan_tab_transfer(&[1, 2, 3], &[7, 8], 2, 2, 1).unwrap();
        assert_eq!(plan.source_index, 1);
        assert_eq!(plan.target_index, 1);
        assert_eq!(plan.target_id, 2);

        assert!(plan_tab_transfer(&[1, 2], &[2, 8], 1, 2, 0).is_none());
        assert!(plan_tab_transfer(&[1, 2, 2], &[8], 2, 3, 0).is_none());
        assert!(plan_tab_transfer(&[1, 2], &[8], 3, 3, 0).is_none());
    }

    #[test]
    fn transfer_plan_clamps_to_the_target_end() {
        let plan = plan_tab_transfer(&[4], &[7, 8], 4, 4, 9).unwrap();
        assert_eq!(plan.target_index, 2);
    }

    #[test]
    fn another_window_in_the_drag_path_remains_a_drop_target() {
        let bounds = gpui::Bounds::new(
            gpui::point(gpui::px(100.0), gpui::px(100.0)),
            gpui::size(gpui::px(200.0), gpui::px(200.0)),
        );
        assert!(is_over_or_moving_toward_bounds(
            bounds,
            gpui::point(gpui::px(120.0), gpui::px(120.0)),
            gpui::point(gpui::px(0.0), gpui::px(0.0)),
        ));
        assert!(is_over_or_moving_toward_bounds(
            bounds,
            gpui::point(gpui::px(80.0), gpui::px(150.0)),
            gpui::point(gpui::px(40.0), gpui::px(150.0)),
        ));
        assert!(!is_over_or_moving_toward_bounds(
            bounds,
            gpui::point(gpui::px(40.0), gpui::px(150.0)),
            gpui::point(gpui::px(80.0), gpui::px(150.0)),
        ));
    }

    #[test]
    fn converts_live_global_pointer_to_target_content_coordinates() {
        let global = gpui::point(gpui::px(540.0), gpui::px(390.0));
        let content_origin = gpui::point(gpui::px(500.0), gpui::px(300.0));

        assert_eq!(
            super::content_local_position(global, content_origin),
            gpui::point(gpui::px(40.0), gpui::px(90.0))
        );
    }

    #[test]
    fn external_drop_target_prefers_favourites_then_tabs_then_window_body() {
        let favourites = gpui::Bounds::new(
            gpui::point(gpui::px(0.0), gpui::px(0.0)),
            gpui::size(gpui::px(40.0), gpui::px(40.0)),
        );
        let strip = gpui::Bounds::new(
            gpui::point(gpui::px(20.0), gpui::px(0.0)),
            gpui::size(gpui::px(60.0), gpui::px(40.0)),
        );
        let viewport = gpui::Bounds::new(
            gpui::point(gpui::px(0.0), gpui::px(0.0)),
            gpui::size(gpui::px(200.0), gpui::px(200.0)),
        );

        assert_eq!(
            external_drop_target_kind(
                gpui::point(gpui::px(25.0), gpui::px(20.0)),
                Some(favourites),
                None,
                Some(strip),
                viewport,
            ),
            Some(ExternalDropTargetKind::Favourites)
        );
        assert_eq!(
            external_drop_target_kind(
                gpui::point(gpui::px(70.0), gpui::px(20.0)),
                None,
                None,
                Some(strip),
                viewport,
            ),
            Some(ExternalDropTargetKind::TabStrip)
        );
        assert_eq!(
            external_drop_target_kind(
                gpui::point(gpui::px(150.0), gpui::px(150.0)),
                None,
                None,
                Some(strip),
                viewport,
            ),
            Some(ExternalDropTargetKind::WindowBody)
        );
    }
}
