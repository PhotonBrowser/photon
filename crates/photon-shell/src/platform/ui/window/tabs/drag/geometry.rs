//! The geometry and planning behind a tab drag: where the pointer is along
//! the tabs, where a dragged tab would land, and what moving it changes.

use super::super::super::super::tabs::TabDragAxis;

pub(super) fn axis_coordinate(point: gpui::Point<gpui::Pixels>, axis: TabDragAxis) -> f32 {
    match axis {
        TabDragAxis::Horizontal => f32::from(point.x),
        TabDragAxis::Vertical => f32::from(point.y),
    }
}

pub(super) fn axis_origin(bounds: gpui::Bounds<gpui::Pixels>, axis: TabDragAxis) -> f32 {
    match axis {
        TabDragAxis::Horizontal => f32::from(bounds.origin.x),
        TabDragAxis::Vertical => f32::from(bounds.origin.y),
    }
}

pub(super) fn axis_size(bounds: gpui::Bounds<gpui::Pixels>, axis: TabDragAxis) -> f32 {
    match axis {
        TabDragAxis::Horizontal => f32::from(bounds.size.width),
        TabDragAxis::Vertical => f32::from(bounds.size.height),
    }
}

pub(super) fn bounds_distance(
    bounds: gpui::Bounds<gpui::Pixels>,
    point: gpui::Point<gpui::Pixels>,
) -> f32 {
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

pub(super) fn is_over_or_moving_toward_bounds(
    bounds: gpui::Bounds<gpui::Pixels>,
    position: gpui::Point<gpui::Pixels>,
    previous_position: gpui::Point<gpui::Pixels>,
) -> bool {
    bounds.contains(&position)
        || bounds_distance(bounds, position) < bounds_distance(bounds, previous_position)
}

pub(super) fn content_local_position(
    global_position: gpui::Point<gpui::Pixels>,
    content_origin: gpui::Point<gpui::Pixels>,
) -> gpui::Point<gpui::Pixels> {
    global_position - content_origin
}

pub(super) fn external_drop_target_kind(
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

pub(super) fn incoming_marker_size(axis: TabDragAxis, size: f32, pinned: bool) -> f32 {
    match axis {
        TabDragAxis::Horizontal if pinned => {
            size.max(super::super::super::super::super::metrics::PINNED_TAB_WIDTH)
        }
        TabDragAxis::Horizontal => {
            size.max(super::super::super::super::super::metrics::TAB_MIN_WIDTH)
        }
        TabDragAxis::Vertical => super::super::super::super::super::metrics::SIDEBAR_TAB_HEIGHT,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TabTransferPlan {
    pub(super) source_index: usize,
    pub(super) target_index: usize,
    pub(super) target_id: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ExternalDropTargetKind {
    Favourites,
    TabStrip,
    WindowBody,
}

pub(super) fn midpoint_insertion_index(
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

pub(super) fn stable_slot_midpoint(visual_origin: f32, extent: f32, visual_offset: f32) -> f32 {
    visual_origin - visual_offset + extent / 2.0
}

pub(super) fn animation_retarget_offset(order_changed: bool, current_offset: f32) -> Option<f32> {
    (order_changed && current_offset.abs() > 0.1).then_some(current_offset)
}

pub(super) fn insert_id_at(order: &mut Vec<u64>, id: u64, index: usize) {
    order.retain(|candidate| *candidate != id);
    order.insert(index.min(order.len()), id);
}

pub(super) fn plan_tab_transfer(
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
