//! Opening, closing and switching tabs, and the tab strip they show in.

use gpui::{
    AnyDrag, App, AppContext, Context, DispatchPhase, HapticFeedbackStyle, Hitbox, KeyDownEvent,
    MouseMoveEvent, Window,
};
use photon_core::Shortcut;
use photon_core::WindowSession;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use super::super::icons::LOADING_SPINNER_STEPS;
use super::super::pages::{NEW_TAB, PageDefinition, find_page};
use super::super::settings::Settings;
use super::super::tabs::{
    DraggedTab, ICON_ENTRANCE, TAB_CLOSE_DURATION, TabDragAxis, TabDragPreview, TabDrop, TabIcon,
    TabPrepaint,
};
use super::super::workspace_layout::WorkspaceLayout;
use super::BrowserWindow;
use super::content::{
    BrowserTab, TabContent, create_webview, create_webview_from_session, follow_appearance,
};
use super::registry::BrowserWindowRegistry;
use crate::platform::app_update::update_app;
use crate::platform::engine::RequestedWebView;
use crate::platform::window_config;

/// A closed tab's page and position, for reopening.
pub(super) struct ClosedTab {
    index: usize,
    url: String,
}

/// How many closed tabs a window remembers.
const CLOSED_TAB_LIMIT: usize = 25;
static NEXT_TAB_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy)]
struct TabInsertionOptions {
    activate: bool,
    animate_appearance: bool,
}

impl BrowserWindow {
    pub(super) fn update_tab_drag(
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
        if layout.phase == super::TabDragPhase::PendingDrag {
            if (position - layout.pointer_start).magnitude()
                < f64::from(super::super::metrics::TAB_DRAG_THRESHOLD)
            {
                return;
            }

            let Some(index) = self.tab_index_by_id(layout.id) else {
                self.tab_drag_layout = None;
                return;
            };
            let tab = &self.tabs[index];
            let bounds = self
                .tab_geometry
                .get(&tab.id)
                .copied()
                .unwrap_or(layout.tab_bounds);
            let dragged = DraggedTab {
                source: window.window_handle(),
                id: tab.id,
                size: match layout.axis {
                    TabDragAxis::Horizontal => f32::from(bounds.size.width),
                    TabDragAxis::Vertical => f32::from(bounds.size.height),
                },
                pinned: layout.pinned,
            };
            let offset = layout.grab_offset;
            let preview = cx.new(|_| TabDragPreview);
            if let Some(layout) = self.tab_drag_layout.as_mut() {
                layout.phase = super::TabDragPhase::Reordering;
                layout.pointer_position = position;
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
        let was_over_favourites = layout.phase == super::TabDragPhase::DraggingOverFavourites;
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
                        let changed = layout.phase != super::TabDragPhase::DraggingOverFavourites
                            || layout.target_window != Some(target_handle);
                        layout.phase = super::TabDragPhase::DraggingOverFavourites;
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
                    self.set_tab_drag_phase(super::TabDragPhase::OutsideTarget, cx);
                    return;
                }
            }
        }
        self.clear_previous_target(cx);
        let in_favourites = self.is_favourites_drop_position(position, cx);
        if in_favourites {
            self.set_favourites_drag_hover(current_window, dragged.id, cx);
            self.set_tab_drag_phase(super::TabDragPhase::DraggingOverFavourites, cx);
            return;
        }
        self.clear_favourites_drag_hover(current_window, dragged.id, cx);

        let in_tab_container = self
            .tab_container_bounds
            .is_some_and(|bounds| bounds.contains(&position));
        if in_tab_container {
            if let Some(layout) = self.tab_drag_layout.as_mut() {
                layout.phase = super::TabDragPhase::Reordering;
            }
            self.reorder_tab_at_pointer(dragged.id, axis, position, cx, window);
            return;
        }

        let moving_toward_favourites = !was_over_favourites
            && self.favourites_drop_bounds(cx).is_some_and(|bounds| {
                bounds_distance(bounds, position) < bounds_distance(bounds, previous_position)
            });
        if moving_toward_favourites {
            self.set_tab_drag_phase(super::TabDragPhase::OutsideTarget, cx);
            return;
        }

        let outside = bounds_distance(hitbox.bounds, position);
        if outside < super::super::metrics::TAB_DETACH_DISTANCE {
            self.set_tab_drag_phase(super::TabDragPhase::OutsideTarget, cx);
            return;
        }
        if !can_detach_tab_count(
            self.tabs.len(),
            Settings::get(cx).close_window_with_last_tab,
        ) {
            self.set_tab_drag_phase(super::TabDragPhase::OutsideTarget, cx);
            return;
        }

        self.set_tab_drag_phase(super::TabDragPhase::Detaching, cx);
        self.detach_tab(dragged, position, window, cx);
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
            TabDragAxis::Horizontal => super::super::metrics::TAB_STRIP_GAP,
            TabDragAxis::Vertical => {
                super::super::metrics::SidebarSpacing::new(self.sidebar.shows_icons()).item_gap
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
                        super::TabPositionAnimation::new(
                            axis,
                            from,
                            super::super::metrics::TAB_REORDER_DURATION_MS,
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
        super::tab_position_animation_offset(self.tab_position_animations.get(&id).copied(), axis)
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

    fn favourites_drop_bounds(&self, cx: &App) -> Option<gpui::Bounds<gpui::Pixels>> {
        self.favourites_bounds
            .or_else(|| self.empty_favourites_reveal_bounds(cx))
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
                component.id == super::super::workspace_layout::ComponentId::FavouritesPanel
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
            2.0 * super::super::metrics::SIDEBAR_SEPARATOR_MARGIN
                + super::super::metrics::MENU_SEPARATOR_HEIGHT,
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

    fn clear_favourites_drag_hover(
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

    fn set_tab_drag_phase(&mut self, phase: super::TabDragPhase, cx: &mut Context<Self>) {
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

    fn notify_other_tab_drop_targets(cx: &mut App, current_window: gpui::AnyWindowHandle) {
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
            layout.phase = super::TabDragPhase::CrossWindowTargeting;
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
                super::TabDragPhase::DraggingOverFavourites,
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
                super::TabDragPhase::CrossWindowTargeting,
                Some(current_window),
                cx,
            );
            return;
        }
        self.clear_incoming_tab_drag(dragged.source, dragged.id, cx);
        self.set_source_drag_phase(
            dragged.source,
            dragged.id,
            super::TabDragPhase::OutsideTarget,
            None,
            cx,
        );
    }

    fn set_source_drag_phase(
        &self,
        source: gpui::AnyWindowHandle,
        id: u64,
        phase: super::TabDragPhase,
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

    fn clear_previous_target(&mut self, cx: &mut App) {
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
        let incoming = super::IncomingTabDrag {
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
        incoming: &super::IncomingTabDrag,
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
            TabDragAxis::Horizontal => super::super::metrics::TAB_STRIP_GAP,
            TabDragAxis::Vertical => {
                super::super::metrics::SidebarSpacing::new(self.sidebar.shows_icons()).item_gap
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
                    super::TabPositionAnimation::new(
                        axis,
                        from,
                        super::super::metrics::TAB_REORDER_DURATION_MS,
                    ),
                );
            } else {
                self.tab_position_animations.remove(&id);
            }
        }
    }

    pub(super) fn finish_tab_drag(
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

        let release_target = self.tab_drag_layout.as_ref().and_then(|drag| {
            (drag.source_window == window.window_handle()
                && matches!(
                    drag.phase,
                    super::TabDragPhase::Reordering | super::TabDragPhase::OutsideTarget
                )
                && self
                    .tab_container_bounds
                    .is_some_and(|bounds| bounds.contains(&release_position))
                && !self
                    .favourites_drop_bounds(cx)
                    .is_some_and(|bounds| bounds.contains(&release_position)))
            .then_some((drag.id, drag.axis))
        });
        if let Some((id, axis)) = release_target {
            self.reorder_tab_at_pointer(id, axis, release_position, cx, window);
        }

        self.clear_previous_target(cx);
        let Some(mut drag) = self.tab_drag_layout.take() else {
            return;
        };
        self.clear_favourites_drag_hover(drag.source_window, drag.id, cx);
        if drag.phase == super::TabDragPhase::PendingDrag {
            return;
        }
        drag.phase = super::TabDragPhase::Completing;
        let current_offset = drag.visual_offset;
        if current_offset.abs() > 0.1 {
            self.tab_position_animations.insert(
                drag.id,
                super::TabPositionAnimation::new(
                    drag.axis,
                    current_offset,
                    super::super::metrics::TAB_DROP_SETTLE_DURATION_MS,
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

    pub(super) fn favourite_drop_handler(&self, cx: &Context<Self>) -> TabDrop {
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

    pub(super) fn favourite_bounds_handler(&self, cx: &Context<Self>) -> TabPrepaint {
        let browser = cx.entity().downgrade();
        Box::new(move |bounds, _, cx| {
            browser
                .update(cx, |this, _| this.favourites_bounds = Some(bounds))
                .ok();
        })
    }

    pub(super) fn tab_container_bounds_handler(&self, cx: &Context<Self>) -> TabPrepaint {
        let browser = cx.entity().downgrade();
        Box::new(move |bounds, window, cx| {
            browser
                .update(cx, |this, cx| {
                    this.tab_container_bounds = Some(bounds);
                    this.auto_scroll_tab_strip(window, cx);
                })
                .ok();
        })
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
                        super::TabDragPhase::PendingDrag
                            | super::TabDragPhase::Completing
                            | super::TabDragPhase::Cancelled
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

        let edge = super::super::metrics::TAB_DRAG_SCROLL_EDGE;
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
            + direction * super::super::metrics::TAB_DRAG_SCROLL_SPEED * proximity * elapsed)
            .clamp(-max_offset, 0.0);
        if (next - current).abs() <= 0.01 {
            self.tab_auto_scroll_at = None;
            return;
        }

        self.tab_strip_scroll
            .set_offset(gpui::point(gpui::px(next), offset.y));
        window.request_animation_frame();
    }

    pub(super) fn tab_target_drop_handler(&self, target_id: u64, cx: &Context<Self>) -> TabDrop {
        let target = cx.entity().downgrade();
        let transfer = self.tab_drop_handler(cx);
        Rc::new(move |dragged, window, cx| {
            if target
                .update(cx, |browser, _| browser.closing_tabs.contains(&target_id))
                .unwrap_or(true)
            {
                return;
            }
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

    pub(super) fn drop_tab_into_today_handler(&self, cx: &Context<Self>) -> TabDrop {
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

    pub(super) fn tab_drop_handler(&self, cx: &Context<Self>) -> TabDrop {
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
                        browser.remove_tab_for_transfer(index, true, source_window, cx);
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
        let Some(index) = self.tab_index_by_id(id) else {
            return;
        };
        if matches!(&self.tabs[index].content, TabContent::Page(_)) {
            return;
        }
        let Some(shortcut) = super::pinned::pin_for(&self.tabs[index], cx) else {
            return;
        };
        if self
            .favourites_drag_hover
            .is_some_and(|(_, hovered_id)| hovered_id == id)
        {
            self.favourites_drag_hover = None;
            cx.notify();
        }
        self.clear_previous_target(cx);
        self.tab_drag_layout = None;
        self.add_favourite(index, shortcut, cx);
    }

    pub(super) fn tab_axis(&self, cx: &App) -> TabDragAxis {
        if WorkspaceLayout::current(cx).saved().tab_layout() == photon_core::TabLayout::Vertical {
            TabDragAxis::Vertical
        } else {
            TabDragAxis::Horizontal
        }
    }

    pub(super) fn tab_drag_active(&self) -> bool {
        self.incoming_tab_drag.is_some()
            || self.tab_drag_layout.as_ref().is_some_and(|drag| {
                !matches!(
                    drag.phase,
                    super::TabDragPhase::PendingDrag
                        | super::TabDragPhase::Completing
                        | super::TabDragPhase::Cancelled
                )
            })
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
        .filter(|&index| {
            self.tabs[index].pin.is_some() == pinned
                && !self.closing_tabs.contains(&self.tabs[index].id)
        })
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

    fn detach_tab(
        &mut self,
        dragged: DraggedTab,
        position: gpui::Point<gpui::Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.tab_index_by_id(dragged.id) else {
            self.cancel_tab_drag(dragged.id, window, cx);
            return;
        };
        if !can_detach_tab_count(
            self.tabs.len(),
            Settings::get(cx).close_window_with_last_tab,
        ) {
            self.set_tab_drag_outside_target(dragged.id, cx);
            return;
        }
        let tab = self.tabs[index].clone();
        let cursor_position = window.inner_window_bounds().get_bounds().origin + position;
        let move_anchor = gpui::point(
            gpui::px(window_config::INITIAL_WIDTH / 2.0),
            gpui::px(window_config::INITIAL_HEIGHT / 2.0),
        );
        let new_window_origin = detached_window_origin(
            cursor_position,
            window_config::INITIAL_WIDTH,
            window_config::INITIAL_HEIGHT,
        );
        let source_display_id = window.display(cx).map(|display| display.id());
        let Ok(new_window) = super::open_browser_window_at(
            self.runtime.clone(),
            None,
            false,
            Some(new_window_origin),
            source_display_id,
            cx,
        ) else {
            self.set_tab_drag_outside_target(dragged.id, cx);
            return;
        };
        let transfer = new_window.update(cx, |browser, window, cx| {
            browser.insert_tab_at_immediately(
                0,
                tab.content.clone(),
                tab.pin.clone(),
                true,
                window,
                cx,
            );
            browser.tabs[0].id = tab.id;
            browser.tabs[0].space = tab.space;
            browser.tabs[0].favourite = tab.favourite.clone();
        });
        if transfer.is_err() {
            let _ = new_window.update(cx, |_, window, _| window.remove_window());
            self.set_tab_drag_outside_target(dragged.id, cx);
            return;
        }

        cx.play_haptic_feedback(HapticFeedbackStyle::Alignment);

        // Detaching hands this physical gesture to the native window manager.
        // Do not start a second GPUI drag over the moving window: some platforms
        // consume the native move's release, leaving that drag active and
        // preventing the newly detached tab from being dragged back afterward.
        let _ = cx.stop_active_drag(window);
        window.release_pointer();
        self.clear_previous_target(cx);
        self.clear_favourites_drag_hover(window.window_handle(), dragged.id, cx);
        self.tab_drag_layout = None;
        self.suppress_tab_click_until = Some(Instant::now() + Duration::from_millis(250));
        self.remove_tab_for_detach(index, window, cx);
        Self::notify_other_tab_drop_targets(cx, window.window_handle());
        let _ = new_window.update(cx, |browser, window, cx| {
            if let Some(index) = browser.tab_index_by_id(tab.id) {
                browser.activate_tab(index, true, window, cx);
            }
            // Keep the center of the new window under the cursor while it moves.
            window.start_window_move_at(move_anchor);
        });
    }

    fn set_tab_drag_outside_target(&mut self, id: u64, cx: &mut Context<Self>) {
        if let Some(drag) = self.tab_drag_layout.as_mut().filter(|drag| drag.id == id) {
            drag.phase = super::TabDragPhase::OutsideTarget;
            drag.target_window = None;
        }
        cx.notify();
    }

    fn cancel_tab_drag(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .tab_drag_layout
            .as_ref()
            .is_some_and(|drag| drag.id == id)
        {
            self.clear_previous_target(cx);
            self.clear_favourites_drag_hover(window.window_handle(), id, cx);
            let _ = cx.stop_active_drag(window);
            window.release_pointer();
            self.tab_drag_layout = None;
            self.tab_position_animations.remove(&id);
            cx.notify();
        }
    }

    fn remove_tab_for_detach(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if !can_detach_tab_count(
            self.tabs.len(),
            Settings::get(cx).close_window_with_last_tab,
        ) {
            return;
        }
        self.remove_tab_for_transfer(index, false, window, cx);
    }

    fn remove_tab_for_transfer(
        &mut self,
        index: usize,
        close_empty_window: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.tabs.len() || self.closing_tabs.contains(&self.tabs[index].id) {
            return;
        }
        let was_active = self.active_tab == Some(index);
        let next = was_active
            .then(|| self.tab_after_closing(index, cx))
            .flatten();
        if let Some(webview) = self.tabs[index].content.webview() {
            webview.update(cx, |view, _| {
                view.session.set_visible(false);
                view.session.set_focus(false);
            });
        }
        self.revealed_icons
            .borrow_mut()
            .remove(&self.tabs[index].id);
        self.tab_visuals.borrow_mut().remove(&self.tabs[index].id);
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
            if should_close_empty_source_window(
                close_empty_window,
                Settings::get(cx).close_window_with_last_tab,
            ) {
                self.record_session_state(WindowSession::default(), cx);
                window.remove_window();
                return;
            } else {
                self.show_empty_new_tab_page(window, cx);
            }
        } else if was_active && let Some(next) = self.active_tab {
            self.activate_tab(next, true, window, cx);
        }
        self.wake_tab_sleep_scheduler();
        cx.notify();
    }

    pub(super) fn activate_tab(
        &mut self,
        index: usize,
        focus_contents: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.tabs.len() || self.closing_tabs.contains(&self.tabs[index].id) {
            return;
        }
        if !self.wake_tab(index) {
            return;
        }
        if self.active_tab != Some(index) {
            // The find bar searches one page; switching tabs ends its search.
            self.close_find_bar(false, window, cx);
            if let Some(previous_index) = self.active_tab {
                if let Some(previous) = self.tabs.get_mut(previous_index) {
                    previous.last_active = Instant::now();
                }
                if let Some(previous) = self
                    .tabs
                    .get(previous_index)
                    .and_then(|tab| tab.content.webview())
                {
                    previous.update(cx, |view, cx| {
                        view.reset_history_swipe(true);
                        view.session.set_visible(false);
                        view.session.set_focus(false);
                        cx.notify();
                    });
                }
            }
            self.active_tab = Some(index);
        }
        self.tabs[index].last_active = Instant::now();
        // A tab of another space brings its space; a favourite's tab
        // belongs to whichever space it is opened in.
        let current = self.current_space(cx);
        if self.tabs[index].favourite.is_some() {
            self.tabs[index].space = current;
        } else if self.shows_spaces(cx) && self.tabs[index].space != current {
            self.switch_space(self.tabs[index].space, cx);
        }
        let webview = self.tabs[index].content.webview();
        if let Some(webview) = webview.as_ref() {
            let window_visible = self.window_visible;
            webview.update(cx, |view, cx| {
                view.session.set_visible(window_visible);
                if focus_contents {
                    window.focus(&view.focus_handle, cx);
                }
                view.track_engine_focus(window, cx);
            });
        }
        self.omnibox.update(cx, |omnibox, cx| {
            omnibox.set_content(webview, self.tabs[index].content.page_address(), window, cx);
            if focus_contents && self.tabs[index].content.webview().is_none() {
                omnibox.focus(window, cx);
            }
        });
        self.sync_dialog(window, cx);
        cx.notify();
        self.wake_tab_sleep_scheduler();
    }

    fn move_tab_focus(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.activate_tab(index, false, window, cx);
        window.focus(&self.tab_focus_handles[index], cx);
    }

    pub(super) fn open_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.insert_page(self.tabs.len(), NEW_TAB, window, cx);
    }

    pub(super) fn insert_tab(
        &mut self,
        index: usize,
        address: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(address) = address {
            if let Some(webview) = create_webview(cx, self.runtime.clone(), address) {
                self.insert_webview(index, webview, None, true, window, cx);
            }
        } else {
            self.insert_page(index, NEW_TAB, window, cx);
        }
    }

    /// Opens `address` in a new tab at the end: one of Photon's pages for a
    /// page's address, otherwise a web page.
    pub(super) fn open_in_new_tab(
        &mut self,
        address: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match find_page(address) {
            Some(page) => self.open_page_tab(page, window, cx),
            None => self.insert_tab(self.tabs.len(), Some(address), window, cx),
        }
    }

    /// Opens one of Photon's pages in a new tab, or switches to the tab
    /// already showing it when the page keeps to one tab.
    pub(super) fn open_page_tab(
        &mut self,
        page: &'static PageDefinition,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let open = page
            .single_tab
            .then(|| {
                self.tabs.iter().position(|tab| {
                    matches!(&tab.content, TabContent::Page(shown) if shown.definition.is(page))
                })
            })
            .flatten();
        match open {
            Some(index) => self.activate_tab(index, true, window, cx),
            None => self.insert_page(self.tabs.len(), page, window, cx),
        }
    }

    /// Opens `address` in a new tab after `opener`'s, as a link opened from
    /// that page does.
    pub(super) fn open_tab_beside(
        &mut self,
        opener: &gpui::Entity<super::super::PhotonWebView>,
        address: &str,
        activate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let index = self
            .tabs
            .iter()
            .position(|tab| tab.content.webview().as_ref() == Some(opener))
            .map_or(self.tabs.len(), |index| index + 1);
        if let Some(webview) = create_webview(cx, self.runtime.clone(), address) {
            self.insert_webview(index, webview, None, activate, window, cx);
        }
    }

    pub(super) fn open_requested_tab(
        &mut self,
        request: RequestedWebView,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let webview = create_webview_from_session(cx, self.runtime.clone(), request.session);
        self.insert_webview(self.tabs.len(), webview, None, request.activate, window, cx);
    }

    /// Inserts a tab showing `webview`, pinned at `pin` or one of today's
    /// tabs. It adopts the window's page size so it lays out and loads even
    /// while in the background.
    pub(super) fn insert_webview(
        &mut self,
        index: usize,
        webview: gpui::Entity<super::super::PhotonWebView>,
        pin: Option<Shortcut>,
        activate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.insert_webview_with_appearance(
            index,
            webview,
            pin,
            TabInsertionOptions {
                activate,
                animate_appearance: true,
            },
            window,
            cx,
        );
    }

    pub(super) fn insert_webview_immediately(
        &mut self,
        index: usize,
        webview: gpui::Entity<super::super::PhotonWebView>,
        pin: Option<Shortcut>,
        activate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.insert_webview_with_appearance(
            index,
            webview,
            pin,
            TabInsertionOptions {
                activate,
                animate_appearance: false,
            },
            window,
            cx,
        );
    }

    fn insert_webview_with_appearance(
        &mut self,
        index: usize,
        webview: gpui::Entity<super::super::PhotonWebView>,
        pin: Option<Shortcut>,
        options: TabInsertionOptions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let viewport = self
            .active_webview()
            .and_then(|webview| webview.read(cx).last_viewport);
        follow_appearance(&webview, window, cx);
        webview.update(cx, |view, _| view.adopt_viewport(viewport));
        let content = TabContent::Web(webview);
        match pin {
            Some(pin) => {
                self.insert_tab_at_with_appearance(index, content, Some(pin), options, window, cx)
            }
            None => {
                let index = index.clamp(self.pinned_count(), self.tabs.len());
                self.insert_tab_at_with_appearance(index, content, None, options, window, cx);
            }
        }
    }

    /// Inserts a tab showing one of Photon's own pages, and switches to it.
    pub(super) fn insert_page(
        &mut self,
        index: usize,
        page: &'static PageDefinition,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let content = self.new_page(page, cx);
        self.insert_content(index, content, true, window, cx);
    }

    /// Inserts a page without a row entrance, for when another container
    /// transition already brings the tab into view.
    pub(super) fn insert_page_immediately(
        &mut self,
        index: usize,
        page: &'static PageDefinition,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let content = self.new_page(page, cx);
        self.insert_tab_at_immediately(index, content, None, true, window, cx);
    }

    /// Inserts a tab of today's tabs, never among the pinned ones.
    pub(super) fn insert_content(
        &mut self,
        index: usize,
        content: TabContent,
        activate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let index = index.clamp(self.pinned_count(), self.tabs.len());
        self.insert_tab_at(index, content, None, activate, window, cx);
    }

    /// Inserts a tab, pinned at `pin` or one of today's tabs.
    pub(super) fn insert_tab_at(
        &mut self,
        index: usize,
        content: TabContent,
        pin: Option<Shortcut>,
        activate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.insert_tab_at_with_appearance(
            index,
            content,
            pin,
            TabInsertionOptions {
                activate,
                animate_appearance: true,
            },
            window,
            cx,
        );
    }

    pub(super) fn insert_tab_at_immediately(
        &mut self,
        index: usize,
        content: TabContent,
        pin: Option<Shortcut>,
        activate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.insert_tab_at_with_appearance(
            index,
            content,
            pin,
            TabInsertionOptions {
                activate,
                animate_appearance: false,
            },
            window,
            cx,
        );
    }

    fn insert_tab_at_with_appearance(
        &mut self,
        index: usize,
        content: TabContent,
        pin: Option<Shortcut>,
        options: TabInsertionOptions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let index = index.min(self.tabs.len());
        let subscription = self.subscribe_to_content(&content, window, cx);
        let id = Self::allocate_tab_id();
        if options.animate_appearance {
            self.tab_entry_animations.borrow_mut().insert(id);
        }
        let space = self.current_space(cx);
        self.tabs
            .insert(index, BrowserTab::new(id, content.clone(), pin, space));
        self.tab_subscriptions.insert(index, subscription);
        self.tab_focus_handles
            .insert(index, cx.focus_handle().tab_stop(self.tabs.len() == 1));
        if self
            .active_tab
            .is_some_and(|active_tab| index <= active_tab && self.tabs.len() > 1)
        {
            // Keep `active_tab` naming the same page until the switch below.
            self.active_tab = self.active_tab.map(|active_tab| active_tab + 1);
        }
        let activate = options.activate || (self.active_tab.is_none() && self.tabs.len() == 1);
        if activate {
            self.empty_page = None;
            self._empty_page_subscription = None;
            self.activate_tab(index, true, window, cx);
        } else {
            if let Some(webview) = content.webview() {
                webview.update(cx, |view, _| {
                    view.session.set_visible(false);
                    view.session.set_focus(false);
                });
            }
            cx.notify();
            self.wake_tab_sleep_scheduler();
        }
    }

    fn allocate_tab_id() -> u64 {
        NEXT_TAB_ID.fetch_add(1, Ordering::Relaxed)
    }

    pub(super) fn reopen_closed_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(closed) = self.closed_tabs.pop() {
            self.insert_tab(closed.index, Some(&closed.url), window, cx);
        }
    }

    /// Activates the tab `offset` places from the active one, wrapping at the ends.
    pub(super) fn select_relative_tab(
        &mut self,
        offset: isize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(index) = self.listed_tab_after(self.active_tab.unwrap_or(0), offset, cx) {
            self.activate_tab(index, true, window, cx);
        }
    }

    fn animate_tab_close_layout(&mut self, closing_id: u64, axis: TabDragAxis, cx: &App) {
        let Some(closing_index) = self.tab_index_by_id(closing_id) else {
            return;
        };
        let pinned = self.tabs[closing_index].pin.is_some();
        let listed: Vec<usize> = match axis {
            TabDragAxis::Horizontal => (0..self.tabs.len()).collect(),
            TabDragAxis::Vertical => self.listed_tabs(cx),
        };
        let gap = match axis {
            TabDragAxis::Horizontal => super::super::metrics::TAB_STRIP_GAP,
            TabDragAxis::Vertical => {
                super::super::metrics::SidebarSpacing::new(self.sidebar.shows_icons()).item_gap
            }
        };

        let animate_group = |this: &mut Self, ids: &[usize], shift: f32| {
            let start = ids
                .iter()
                .filter_map(|&index| {
                    let id = this.tabs[index].id;
                    this.tab_geometry
                        .get(&id)
                        .copied()
                        .map(|bounds| axis_origin(bounds, axis) - this.tab_offset(id, axis))
                })
                .fold(f32::INFINITY, f32::min);
            if !start.is_finite() {
                return;
            }

            let mut cursor = start - shift;
            for &index in ids {
                let id = this.tabs[index].id;
                if this.closing_tabs.contains(&id) {
                    continue;
                }
                let Some(bounds) = this.tab_geometry.get(&id).copied() else {
                    continue;
                };
                let from = axis_origin(bounds, axis) - cursor;
                match animation_retarget_offset(true, from) {
                    Some(from) => {
                        this.tab_position_animations.insert(
                            id,
                            super::TabPositionAnimation::new(
                                axis,
                                from,
                                super::super::metrics::TAB_REORDER_DURATION_MS,
                            ),
                        );
                    }
                    None => {
                        this.tab_position_animations.remove(&id);
                    }
                }
                cursor += axis_size(bounds, axis) + gap;
            }
        };

        if axis == TabDragAxis::Horizontal {
            animate_group(self, &listed, 0.0);
            return;
        }

        let pinned_ids: Vec<usize> = listed
            .iter()
            .copied()
            .filter(|&index| self.tabs[index].pin.is_some())
            .collect();
        let today_ids: Vec<usize> = listed
            .iter()
            .copied()
            .filter(|&index| self.tabs[index].pin.is_none())
            .collect();
        animate_group(self, &pinned_ids, 0.0);

        let today_shift = if pinned {
            let has_live_pinned = pinned_ids
                .iter()
                .any(|&index| !self.closing_tabs.contains(&self.tabs[index].id));
            let extent = self
                .closing_tab_extents
                .get(&closing_id)
                .copied()
                .unwrap_or_default();
            if has_live_pinned {
                extent + gap
            } else {
                let spacing =
                    super::super::metrics::SidebarSpacing::new(self.sidebar.shows_icons());
                let divider_extent =
                    2.0 * spacing.separator_margin + super::super::metrics::MENU_SEPARATOR_HEIGHT;
                extent + divider_extent + 2.0 * spacing.item_gap
            }
        } else {
            0.0
        };
        animate_group(self, &today_ids, today_shift);
    }

    pub(super) fn close_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len() || self.closing_tabs.contains(&self.tabs[index].id) {
            return;
        }
        let animations = Settings::get(cx).animations;
        let id = self.tabs[index].id;
        let closes_last_window_tab =
            self.tabs.len() == 1 && Settings::get(cx).close_window_with_last_tab;
        let axis = self.tab_axis(cx);
        let extent = self.tab_geometry.get(&id).map(|bounds| match axis {
            TabDragAxis::Horizontal => f32::from(bounds.size.width),
            TabDragAxis::Vertical => f32::from(bounds.size.height),
        });
        let can_animate = !closes_last_window_tab
            && animations.enabled
            && !cx.reduce_motion()
            && extent.is_some_and(|extent| extent > 0.0);

        if !can_animate {
            self.finish_close_tab(index, window, cx);
            return;
        }

        let has_other_tab = match axis {
            TabDragAxis::Horizontal => self
                .tabs
                .iter()
                .filter(|tab| tab.id != id)
                .any(|tab| !self.closing_tabs.contains(&tab.id)),
            TabDragAxis::Vertical => self.listed_tabs(cx).into_iter().any(|other| {
                other != index
                    && self.tabs[other].pin.is_some() == self.tabs[index].pin.is_some()
                    && !self.closing_tabs.contains(&self.tabs[other].id)
            }),
        };
        let gap = if has_other_tab {
            match axis {
                TabDragAxis::Horizontal => super::super::metrics::TAB_STRIP_GAP,
                TabDragAxis::Vertical => {
                    super::super::metrics::SidebarSpacing::new(self.sidebar.shows_icons()).item_gap
                }
            }
        } else {
            0.0
        };

        let cancel_drag = self
            .tab_drag_layout
            .as_ref()
            .is_some_and(|drag| drag.id == id);
        if cancel_drag {
            if let Some(drag) = self.tab_drag_layout.as_mut() {
                drag.phase = super::TabDragPhase::Cancelled;
            }
            self.clear_previous_target(cx);
            self.clear_favourites_drag_hover(window.window_handle(), id, cx);
            let _ = cx.stop_active_drag(window);
            window.release_pointer();
            self.tab_drag_layout = None;
            self.tab_position_animations.remove(&id);
            Self::notify_other_tab_drop_targets(cx, window.window_handle());
        }
        let was_active = self.active_tab == Some(index);
        self.closing_tabs.insert(id);
        self.wake_tab_sleep_scheduler();
        if was_active {
            self.closing_tab_active.insert(id);
        }
        self.closing_tab_extents
            .insert(id, extent.unwrap_or_default());
        self.closing_tab_gaps.insert(id, gap);
        self.animate_tab_close_layout(id, axis, cx);
        if was_active {
            if let Some(next) = self.tab_to_activate_after_closing(index, cx) {
                self.activate_tab(next, true, window, cx);
            } else {
                if let Some(webview) = self.tabs[index].content.webview() {
                    webview.update(cx, |view, _| {
                        view.session.set_visible(false);
                        view.session.set_focus(false);
                    });
                }
                self.active_tab = None;
                self.close_find_bar(false, window, cx);
                self.omnibox.update(cx, |omnibox, cx| {
                    omnibox.set_content(None, None, window, cx)
                });
                self.show_empty_new_tab_page(window, cx);
            }
        }
        cx.notify();

        let handle = window.window_handle();
        let close_task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TAB_CLOSE_DURATION).await;
            update_app(cx, |cx| {
                handle
                    .update(cx, |_, window, cx| {
                        this.update(cx, |this, cx| {
                            this.tab_close_tasks.remove(&id);
                            if let Some(index) = this.tab_index_by_id(id)
                                && this.closing_tabs.contains(&id)
                            {
                                this.finish_close_tab(index, window, cx);
                            }
                        })
                    })
                    .ok()
            })
            .await;
        });
        self.tab_close_tasks.insert(id, close_task);
    }

    fn finish_close_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }
        let removed = self.tabs[index].clone();
        let closes_window = self.tabs.len() == 1 && Settings::get(cx).close_window_with_last_tab;
        if let Some(webview) = removed.content.webview() {
            let favicon = webview.update(cx, |view, _| {
                view.session.set_visible(false);
                view.session.set_focus(false);
                view.favicon.take()
            });
            if let Some(favicon) = favicon {
                cx.drop_image(favicon.image, Some(window));
            }
        }
        self.revealed_icons.borrow_mut().remove(&removed.id);
        self.tab_visuals.borrow_mut().remove(&removed.id);
        self.tab_entry_animations.borrow_mut().remove(&removed.id);
        self.tab_position_animations.remove(&removed.id);
        self.tab_geometry.remove(&removed.id);
        self.closing_tabs.remove(&removed.id);
        self.closing_tab_active.remove(&removed.id);
        self.closing_tab_extents.remove(&removed.id);
        self.closing_tab_gaps.remove(&removed.id);
        self.tab_close_tasks.remove(&removed.id);
        self.wake_tab_sleep_scheduler();

        if closes_window {
            self.record_session_state(WindowSession::default(), cx);
            window.remove_window();
            return;
        }
        self.remember_closed(index, &removed.content, cx);

        let was_active = self.active_tab == Some(index);
        let next = was_active
            .then(|| self.tab_after_closing(index, cx))
            .flatten();
        let closed = self.tabs.remove(index);
        drop(self.tab_subscriptions.remove(index));
        self.tab_focus_handles.remove(index);
        if let Some(active_tab) = self.active_tab {
            if index < active_tab {
                self.active_tab = Some(active_tab - 1);
            } else if was_active {
                self.active_tab = next;
            }
        }

        if closed.pin.is_some() {
            self.forget_pin(&closed, cx);
        }
        if self.tabs.is_empty() {
            self.active_tab = None;
            self.close_find_bar(false, window, cx);
            self.omnibox.update(cx, |omnibox, cx| {
                omnibox.set_content(None, None, window, cx)
            });
            if self.empty_page.is_none() {
                self.show_empty_new_tab_page(window, cx);
            }
            cx.notify();
            return;
        }
        if was_active {
            match self.active_tab {
                Some(active_tab) => self.activate_tab(active_tab, true, window, cx),
                // The space has no other tab; show it a new tab page, as an
                // empty space does when switched to.
                None if self.closing_tabs.is_empty() => {
                    self.insert_page(self.tabs.len(), NEW_TAB, window, cx)
                }
                None => {}
            }
        } else if self.active_tab.is_none() && self.closing_tabs.is_empty() {
            self.insert_page(self.tabs.len(), NEW_TAB, window, cx);
        } else {
            cx.notify();
        }
    }

    /// The next existing tab to activate while `index` is still mounted.
    fn tab_to_activate_after_closing(&self, index: usize, cx: &Context<Self>) -> Option<usize> {
        let listed = self.listed_tabs(cx);
        if let Some(position) = listed.iter().position(|&listed| listed == index) {
            listed
                .iter()
                .skip(position + 1)
                .chain(listed[..position].iter().rev())
                .copied()
                .find(|&candidate| {
                    candidate != index && !self.closing_tabs.contains(&self.tabs[candidate].id)
                })
        } else {
            listed
                .into_iter()
                .filter(|&candidate| {
                    candidate != index && !self.closing_tabs.contains(&self.tabs[candidate].id)
                })
                .max_by_key(|&candidate| self.tabs[candidate].last_active)
        }
    }

    /// The tab to show once the active tab at `index` closes, by its index
    /// after the close: its neighbour in the list, after it or else before
    /// it, so closing a tab never moves to another space. A tab not listed,
    /// such as a favourite's, gives way to the list's most recent tab.
    /// `None` when the list has no other tab.
    fn tab_after_closing(&self, index: usize, cx: &Context<Self>) -> Option<usize> {
        let listed = self.listed_tabs(cx);
        let next = match listed.iter().position(|&listed| listed == index) {
            Some(position) => listed
                .iter()
                .skip(position + 1)
                .chain(listed[..position].iter().rev())
                .copied()
                .find(|&candidate| {
                    candidate != index && !self.closing_tabs.contains(&self.tabs[candidate].id)
                }),
            None => listed
                .into_iter()
                .filter(|&candidate| {
                    candidate != index && !self.closing_tabs.contains(&self.tabs[candidate].id)
                })
                .max_by_key(|&candidate| self.tabs[candidate].last_active),
        };
        next.map(|next| if next > index { next - 1 } else { next })
    }

    /// Keeps a closed web page so it can be reopened.
    fn remember_closed(&mut self, index: usize, content: &TabContent, cx: &Context<Self>) {
        let url = match content {
            TabContent::Sleeping(sleeping) => {
                let view = sleeping.webview.read(cx);
                view.has_page().then(|| view.state.url.clone())
            }
            TabContent::Web(webview) => {
                let view = webview.read(cx);
                view.has_page().then(|| view.state.url.clone())
            }
            TabContent::Page(_) => None,
        };
        let Some(url) = url else { return };
        if self.closed_tabs.len() == CLOSED_TAB_LIMIT {
            self.closed_tabs.remove(0);
        }
        self.closed_tabs.push(ClosedTab { index, url });
    }

    /// Whether a loading tab or a notice shows a spinner.
    fn spinner_shown(&self, cx: &Context<Self>) -> bool {
        self.notice_is_working(cx)
            || self
                .tabs
                .iter()
                .filter_map(|tab| tab.content.webview())
                .any(|webview| {
                    let view = webview.read(cx);
                    view.state.loading && view.has_page()
                })
    }

    /// Whether any tab, and so any spinner, is on screen: in a tab strip
    /// Zen mode has not tucked away, or in a sidebar that is shown or
    /// revealed. Showing them again redraws them at the current step.
    fn tabs_on_screen(&self, cx: &App) -> bool {
        let layout = WorkspaceLayout::current(cx);
        (layout.has_tab_strip() && self.sidebar.chrome_shown() > 0.0)
            || (layout.has_sidebar() && (self.sidebar.shown() > 0.0 || self.sidebar.over_page()))
    }

    /// Steps the spinners while any is shown. With reduced motion they stay still.
    pub(super) fn animate_spinner(&mut self, cx: &mut Context<Self>) {
        let animations = Settings::get(cx).animations;
        if self.spinner_running
            || cx.reduce_motion()
            || !animations.enabled
            || !self.spinner_shown(cx)
        {
            return;
        }
        self.spinner_running = true;
        let interval = Duration::from_millis(u64::from(animations.spinner_step_interval_ms()));
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(interval).await;
                let keep_running = update_app(cx, |cx| {
                    this.update(cx, |this, cx| {
                        let keep_running = !cx.reduce_motion()
                            && Settings::get(cx).animations.enabled
                            && this.spinner_shown(cx);
                        if keep_running {
                            this.spinner_step = (this.spinner_step + 1) % LOADING_SPINNER_STEPS;
                            if this.window_visible && this.tabs_on_screen(cx) {
                                this.chrome.update(cx, |_, cx| cx.notify());
                                if this.notice_is_working(cx) {
                                    cx.notify();
                                }
                            }
                        } else {
                            this.spinner_running = false;
                        }
                        keep_running
                    })
                    .ok()
                })
                .await
                .flatten()
                .unwrap_or(false);
                if !keep_running {
                    break;
                }
            }
        })
        .detach();
    }

    /// Up and down arrows move between tabs from a focused tab.
    pub(super) fn tab_key_down(
        &mut self,
        tab_id: u64,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.tab_index_by_id(tab_id) else {
            return;
        };
        if self.closing_tabs.contains(&tab_id) {
            return;
        }
        if event.keystroke.modifiers.modified() {
            return;
        }
        let offset = match event.keystroke.key.as_str() {
            "up" => -1,
            "down" => 1,
            _ => return,
        };
        if let Some(next_index) = self.listed_tab_after(index, offset, cx) {
            self.move_tab_focus(next_index, window, cx);
        }
        window.prevent_default();
        cx.stop_propagation();
    }

    /// Whether a tab's icon is still within its appear animation. A newly
    /// revealed icon starts it; the same icon shown again does not.
    pub(super) fn icon_appearing(
        &self,
        tab: u64,
        icon: &TabIcon,
        animations: &photon_core::AnimationSettings,
    ) -> bool {
        let Some(revealed) = icon.revealed() else {
            return false;
        };
        let mut revealed_icons = self.revealed_icons.borrow_mut();
        match revealed_icons.get(&tab) {
            Some((icon, since)) if *icon == revealed => {
                since.elapsed() < ICON_ENTRANCE.total_duration(animations)
            }
            _ => {
                revealed_icons.insert(tab, (revealed, Instant::now()));
                true
            }
        }
    }

    /// Whether a newly inserted tab is within its one-time entry animation.
    pub(super) fn tab_entry_appearing(
        &self,
        tab: u64,
        animations: &photon_core::AnimationSettings,
    ) -> bool {
        let mut entries = self.tab_entry_animations.borrow_mut();
        if !animations.enabled {
            entries.remove(&tab);
            return false;
        }
        entries.contains(&tab)
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

fn detached_window_origin(
    cursor_position: gpui::Point<gpui::Pixels>,
    window_width: f32,
    window_height: f32,
) -> gpui::Point<gpui::Pixels> {
    cursor_position - gpui::point(gpui::px(window_width / 2.0), gpui::px(window_height / 2.0))
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
        TabDragAxis::Horizontal if pinned => size.max(super::super::metrics::PINNED_TAB_WIDTH),
        TabDragAxis::Horizontal => size.max(super::super::metrics::TAB_MIN_WIDTH),
        TabDragAxis::Vertical => super::super::metrics::SIDEBAR_TAB_HEIGHT,
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

fn can_detach_tab_count(tab_count: usize, close_window_with_last_tab: bool) -> bool {
    tab_count > 1 || (tab_count == 1 && !close_window_with_last_tab)
}

fn should_close_empty_source_window(
    close_source_after_transfer: bool,
    close_window_with_last_tab: bool,
) -> bool {
    close_source_after_transfer || close_window_with_last_tab
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
    use super::super::{TabPositionAnimation, tab_position_animation_offset};
    use super::TabDragAxis;
    use super::{
        ExternalDropTargetKind, animation_retarget_offset, can_detach_tab_count,
        detached_window_origin, external_drop_target_kind, insert_id_at,
        is_over_or_moving_toward_bounds, midpoint_insertion_index, plan_tab_transfer,
        should_close_empty_source_window, stable_slot_midpoint,
    };

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
        let animation = TabPositionAnimation::new(TabDragAxis::Horizontal, 24.0, 1_000);
        assert!(tab_position_animation_offset(Some(animation), TabDragAxis::Horizontal) > 0.0);
        assert_eq!(
            tab_position_animation_offset(Some(animation), TabDragAxis::Vertical),
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
    fn detaching_the_last_tab_follows_the_window_close_setting() {
        assert!(!can_detach_tab_count(0, false));
        assert!(!can_detach_tab_count(0, true));
        assert!(can_detach_tab_count(1, false));
        assert!(!can_detach_tab_count(1, true));
        assert!(can_detach_tab_count(2, false));
        assert!(can_detach_tab_count(2, true));
    }

    #[test]
    fn transferring_the_last_tab_to_another_window_closes_the_empty_source() {
        assert!(should_close_empty_source_window(true, false));
        assert!(should_close_empty_source_window(true, true));
        assert!(should_close_empty_source_window(false, true));
        assert!(!should_close_empty_source_window(false, false));
    }

    #[test]
    fn another_window_in_the_drag_path_delays_detachment() {
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
    fn detached_window_centers_on_the_drag_pointer() {
        let cursor = gpui::point(gpui::px(540.0), gpui::px(390.0));
        let width = 640.0;
        let height = 480.0;
        let origin = detached_window_origin(cursor, width, height);

        assert_eq!(
            origin + gpui::point(gpui::px(width / 2.0), gpui::px(height / 2.0)),
            cursor,
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
