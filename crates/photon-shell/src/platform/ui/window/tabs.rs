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
    DraggedTab, ICON_ENTRANCE, TabDragAxis, TabDragPreview, TabDrop, TabIcon, TabPrepaint,
};
use super::super::workspace_layout::WorkspaceLayout;
use super::BrowserWindow;
use super::content::{
    BrowserTab, TabContent, create_webview, create_webview_from_session, follow_appearance,
};
use crate::platform::app_update::update_app;
use crate::platform::engine::RequestedWebView;

/// A closed tab's page and position, for reopening.
pub(super) struct ClosedTab {
    index: usize,
    url: String,
}

/// How many closed tabs a window remembers.
const CLOSED_TAB_LIMIT: usize = 25;
static NEXT_TAB_ID: AtomicU64 = AtomicU64::new(1);

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
        if layout.axis != self.tab_axis(cx) {
            self.clear_previous_target(cx);
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
                label: layout.label.clone().into(),
                axis: layout.axis,
                size: match layout.axis {
                    TabDragAxis::Horizontal => f32::from(bounds.size.width),
                    TabDragAxis::Vertical => f32::from(bounds.size.height),
                },
                pinned: layout.pinned,
            };
            let offset = layout.grab_offset;
            let palette = self.palette(window, cx);
            let preview_label = dragged.label.clone();
            let preview = cx.new(move |_| TabDragPreview {
                label: preview_label,
                palette,
            });
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
        let in_favourites = self.is_favourites_drop_position(position, cx);
        if in_favourites {
            self.clear_previous_target(cx);
            self.set_favourites_drag_hover(current_window, dragged.id, cx);
            self.set_tab_drag_phase(super::TabDragPhase::DraggingOverFavourites, cx);
            return;
        }
        self.clear_favourites_drag_hover(current_window, dragged.id, cx);

        let global_position = window.bounds().origin + position;
        if let Some((target, over_favourites, target_position)) =
            self.external_drop_target(global_position, current_window, cx)
        {
            if over_favourites {
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
                let mut changed = false;
                if let Some(layout) = self.tab_drag_layout.as_mut() {
                    changed = layout.phase != super::TabDragPhase::DraggingOverFavourites
                        || layout.target_window != Some(target_handle);
                    layout.phase = super::TabDragPhase::DraggingOverFavourites;
                    layout.target_window = Some(target_handle);
                }
                if changed {
                    cx.notify();
                }
                return;
            }
            self.set_cross_window_target(target, &dragged, target_position, cx);
            return;
        }
        self.clear_previous_target(cx);

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
        if !can_detach_tab_count(self.tabs.len()) {
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
        let _ = axis;
        self.tab_position_animations
            .get(&id)
            .map(|animation| animation.value())
            .unwrap_or(0.0)
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
        current_window: gpui::AnyWindowHandle,
        cx: &mut Context<Self>,
    ) -> Option<(
        gpui::WindowHandle<BrowserWindow>,
        bool,
        gpui::Point<gpui::Pixels>,
    )> {
        let mut strip_target = None;
        for handle in cx.windows() {
            if handle.window_id() == current_window.window_id() {
                continue;
            }
            let Some(target) = handle.downcast::<BrowserWindow>() else {
                continue;
            };
            let Ok(window_bounds) = target.update(cx, |_, window, _| window.bounds()) else {
                continue;
            };
            let local_position = global_position - window_bounds.origin;
            let Ok((favourites, tab_container, reveal_bounds)) = target.read(cx).map(|browser| {
                (
                    browser.favourites_bounds,
                    browser.tab_container_bounds,
                    browser.favourites_drop_bounds(cx),
                )
            }) else {
                continue;
            };
            if favourites.is_some_and(|bounds| bounds.contains(&local_position))
                || reveal_bounds.is_some_and(|bounds| bounds.contains(&local_position))
            {
                return Some((target, true, local_position));
            }
            if strip_target.is_none()
                && tab_container.is_some_and(|bounds| bounds.contains(&local_position))
            {
                strip_target = Some((target, false, local_position));
            }
        }
        strip_target
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
            return;
        }
        let incoming = super::IncomingTabDrag {
            source: dragged.source,
            id: dragged.id,
            axis,
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
                        from,
                        super::super::metrics::TAB_REORDER_DURATION_MS,
                    ),
                );
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
        Box::new(move |bounds, _, cx| {
            browser
                .update(cx, |this, _| this.tab_container_bounds = Some(bounds))
                .ok();
        })
    }

    pub(super) fn tab_target_drop_handler(&self, target_id: u64, cx: &Context<Self>) -> TabDrop {
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
                    browser.insert_tab_at(
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
        let Some(index) = self.tab_index_by_id(id) else {
            return;
        };
        let Some(view) = self.tabs[index].content.webview() else {
            return;
        };
        let shortcut = {
            let view = view.read(cx);
            if !view.has_page() {
                return;
            }
            Shortcut {
                title: view.state.title.clone(),
                url: view.state.url.clone(),
            }
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

    fn tab_axis(&self, cx: &App) -> TabDragAxis {
        if WorkspaceLayout::current(cx).saved().tab_layout() == photon_core::TabLayout::Vertical {
            TabDragAxis::Vertical
        } else {
            TabDragAxis::Horizontal
        }
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
        if !can_detach_tab_count(self.tabs.len()) {
            self.set_tab_drag_outside_target(dragged.id, cx);
            return;
        }
        let tab = self.tabs[index].clone();
        let source_bounds = window.bounds();
        let source_tab_bounds = self
            .tab_geometry
            .get(&dragged.id)
            .copied()
            .unwrap_or_default();
        let grab_offset = self
            .tab_drag_layout
            .as_ref()
            .map(|layout| layout.grab_offset)
            .unwrap_or_default();
        let new_tab_origin = match dragged.axis {
            TabDragAxis::Horizontal => gpui::point(
                gpui::px(
                    super::super::metrics::WINDOW_CONTROLS_INSET
                        + super::super::metrics::TAB_STRIP_INSET,
                ),
                gpui::px(super::super::metrics::TAB_STRIP_INSET),
            ),
            TabDragAxis::Vertical => source_tab_bounds.origin,
        };
        let new_window_origin = source_bounds.origin + position - new_tab_origin - grab_offset;
        let Ok(new_window) = super::open_browser_window_at(
            self.runtime.clone(),
            None,
            false,
            Some(new_window_origin),
            cx,
        ) else {
            self.set_tab_drag_outside_target(dragged.id, cx);
            return;
        };
        let transfer = new_window.update(cx, |browser, window, cx| {
            browser.insert_tab_at(0, tab.content.clone(), tab.pin.clone(), true, window, cx);
            browser.tabs[0].id = tab.id;
            browser.tabs[0].space = tab.space;
            browser.tabs[0].favourite = tab.favourite.clone();
            browser.tab_drag_layout = Some(super::TabDragLayout {
                id: tab.id,
                source_window: window.window_handle(),
                axis: dragged.axis,
                pointer_start: position,
                pointer_position: position,
                grab_offset,
                tab_bounds: source_tab_bounds,
                label: dragged.label.to_string(),
                pinned: dragged.pinned,
                phase: super::TabDragPhase::DraggingDetachedWindow,
                visual_offset: 0.0,
                target_window: None,
            });
            let palette = browser.palette(window, cx);
            let preview_label = dragged.label.clone();
            let preview = cx.new(move |_| TabDragPreview {
                label: preview_label,
                palette,
            });
            window.start_window_move();
            preview
        });
        let Ok(preview) = transfer else {
            let _ = new_window.update(cx, |_, window, _| window.remove_window());
            self.set_tab_drag_outside_target(dragged.id, cx);
            return;
        };

        cx.play_haptic_feedback(HapticFeedbackStyle::Alignment);
        let new_payload = DraggedTab {
            source: new_window.into(),
            id: tab.id,
            label: dragged.label,
            axis: dragged.axis,
            size: dragged.size,
            pinned: dragged.pinned,
        };
        let _ = cx.stop_drag(window);
        cx.start_drag(AnyDrag::new(new_payload, preview).offset(grab_offset));
        self.tab_drag_layout = None;
        self.suppress_tab_click_until = Some(Instant::now() + Duration::from_millis(250));
        self.remove_tab_for_detach(index, window, cx);
        Self::notify_other_tab_drop_targets(cx, window.window_handle());
        let _ = new_window.update(cx, |browser, window, cx| {
            if let Some(index) = browser.tab_index_by_id(tab.id) {
                browser.activate_tab(index, true, window, cx);
            }
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
        if !can_detach_tab_count(self.tabs.len()) {
            return;
        }
        self.remove_tab_for_transfer(index, window, cx);
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
                view.session.set_visible(false);
                view.session.set_focus(false);
            });
        }
        self.revealed_icons
            .borrow_mut()
            .remove(&self.tabs[index].id);
        self.tab_position_animations.remove(&self.tabs[index].id);
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
            if Settings::get(cx).close_window_with_last_tab {
                self.record_session_state(WindowSession::default(), cx);
                window.remove_window();
                return;
            } else {
                self.show_empty_new_tab_page(window, cx);
            }
        } else if was_active {
            if let Some(next) = self.active_tab {
                self.activate_tab(next, true, window, cx);
            }
        }
        cx.notify();
    }

    pub(super) fn activate_tab(
        &mut self,
        index: usize,
        focus_contents: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.tabs.len() {
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
        let viewport = self
            .active_webview()
            .and_then(|webview| webview.read(cx).last_viewport);
        follow_appearance(&webview, window, cx);
        webview.update(cx, |view, _| view.adopt_viewport(viewport));
        let content = TabContent::Web(webview);
        match pin {
            Some(pin) => self.insert_tab_at(index, content, Some(pin), activate, window, cx),
            None => self.insert_content(index, content, activate, window, cx),
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
        let index = index.min(self.tabs.len());
        let subscription = self.subscribe_to_content(&content, window, cx);
        let id = Self::allocate_tab_id();
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
        let activate = activate || (self.active_tab.is_none() && self.tabs.len() == 1);
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

    pub(super) fn close_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }

        let removed = self.tabs[index].clone();
        let cancel_drag = self
            .tab_drag_layout
            .as_ref()
            .is_some_and(|drag| drag.id == removed.id);
        if cancel_drag {
            if let Some(drag) = self.tab_drag_layout.as_mut() {
                drag.phase = super::TabDragPhase::Cancelled;
            }
            self.clear_previous_target(cx);
            let _ = cx.stop_active_drag(window);
            window.release_pointer();
            self.tab_drag_layout = None;
            self.tab_position_animations.remove(&removed.id);
            Self::notify_other_tab_drop_targets(cx, window.window_handle());
        }
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

        if self.tabs.len() == 1 && Settings::get(cx).close_window_with_last_tab {
            // Closed with its last tab, the window has nothing to reopen.
            self.record_session_state(WindowSession::default(), cx);
            window.remove_window();
            return;
        }
        self.remember_closed(index, &removed.content, cx);

        let was_active = self.active_tab == Some(index);
        let next = if was_active {
            self.tab_after_closing(index, cx)
        } else {
            None
        };
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
            self.show_empty_new_tab_page(window, cx);
            cx.notify();
            return;
        }
        if was_active {
            match self.active_tab {
                Some(active_tab) => self.activate_tab(active_tab, true, window, cx),
                // The space has no other tab; show it a new tab page, as an
                // empty space does when switched to.
                None => self.insert_page(self.tabs.len(), NEW_TAB, window, cx),
            }
        } else {
            cx.notify();
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
                .get(position + 1)
                .or_else(|| listed.get(position.checked_sub(1)?))
                .copied(),
            None => listed
                .iter()
                .copied()
                .max_by_key(|&listed| self.tabs[listed].last_active),
        };
        next.map(|next| if next > index { next - 1 } else { next })
    }

    /// Keeps a closed web page so it can be reopened.
    fn remember_closed(&mut self, index: usize, content: &TabContent, cx: &Context<Self>) {
        let Some(webview) = content.webview() else {
            return;
        };
        let view = webview.read(cx);
        if view.has_page() {
            if self.closed_tabs.len() == CLOSED_TAB_LIMIT {
                self.closed_tabs.remove(0);
            }
            self.closed_tabs.push(ClosedTab {
                index,
                url: view.state.url.clone(),
            });
        }
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
        index: usize,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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

fn can_detach_tab_count(tab_count: usize) -> bool {
    tab_count > 1
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
    use super::{
        animation_retarget_offset, can_detach_tab_count, insert_id_at, midpoint_insertion_index,
        plan_tab_transfer, stable_slot_midpoint,
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
    fn a_window_with_one_tab_cannot_detach_it() {
        assert!(!can_detach_tab_count(0));
        assert!(!can_detach_tab_count(1));
        assert!(can_detach_tab_count(2));
    }
}
