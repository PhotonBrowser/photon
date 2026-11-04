//! Full-width and collapsed tab rows for the sidebar.

use std::time::Duration;

use gpui::{AnyElement, InteractiveElement, div, prelude::*, px, rgba};

use super::super::super::motion::SurfaceFade;
use super::super::super::sidebar::collapsed_tile;
use super::super::super::tooltip::label_tooltip;
use super::super::super::{metrics, theme::ThemeColors};
use super::super::{ReorderGroup, TabDragAxis, TabParts, animate_tab_motion, measured_tab};

/// A full-width row with its title and a close button.
pub(super) fn tab_row(
    parts: TabParts,
    palette: ThemeColors,
    reorder: &ReorderGroup<u64>,
) -> AnyElement {
    let motion = parts.motion;
    let position_offset = parts.position_offset;
    let reorder_key = parts.reorder_key;
    let focus_handle = parts.focus_handle.clone();
    let reorder_transition = parts.reorder_transition;
    let row = parts
        .tab
        .w_full()
        .gap(px(metrics::SIDEBAR_TAB_ICON_GAP))
        .h(px(metrics::SIDEBAR_TAB_HEIGHT))
        .pl(px(metrics::SIDEBAR_TAB_PADDING))
        .pr(px(metrics::SIDEBAR_PADDING))
        .rounded(px(metrics::sidebar_item_radius()))
        .text_size(px(metrics::SIDEBAR_FONT_SIZE));
    let tooltip = (!parts.suppress_tooltips).then(|| label_tooltip(parts.label.clone(), palette));
    let row = tab_surface(
        row.when_some(tooltip, |row, tooltip| row.tooltip(tooltip)),
        parts.shown,
        palette,
    );
    let close = if parts.active {
        parts.close
    } else {
        // Hidden, it is also left out of the tab order, so keyboard focus
        // never lands on it unseen.
        parts
            .close
            .tab_stop(false)
            .opacity(0.0)
            .group_hover_style(parts.group, |style| style.opacity(1.0))
    };
    let row = animate_tab_motion(
        row.child(parts.icon)
            .child(div().flex_1().min_w_0().truncate().child(parts.label))
            .child(close),
        motion,
        TabDragAxis::Vertical,
        position_offset,
    );
    measured_tab(row, reorder_key, focus_handle, reorder_transition, reorder)
}

/// A square icon tile for the sidebar when it is collapsed.
pub(super) fn tab_tile(
    parts: TabParts,
    palette: ThemeColors,
    reorder: &ReorderGroup<u64>,
) -> AnyElement {
    let motion = parts.motion;
    let position_offset = parts.position_offset;
    let reorder_key = parts.reorder_key;
    let focus_handle = parts.focus_handle.clone();
    let reorder_transition = parts.reorder_transition;
    let tooltip = (!parts.suppress_tooltips).then(|| label_tooltip(parts.label.clone(), palette));
    let tile = collapsed_tile(parts.tab)
        .relative()
        .justify_center()
        .when_some(tooltip, |tile, tooltip| tile.tooltip(tooltip));
    let tile = tab_surface(tile, parts.shown, palette);
    let tile = animate_tab_motion(
        tile.child(parts.icon),
        motion,
        TabDragAxis::Vertical,
        position_offset,
    );
    measured_tab(tile, reorder_key, focus_handle, reorder_transition, reorder)
}

fn tab_surface<E: InteractiveElement + gpui::Styled>(
    element: E,
    shown: bool,
    palette: ThemeColors,
) -> E {
    let element = if shown {
        element.surface_fade().bg(rgba(palette.selected_surface))
    } else {
        element
            .surface_fade()
            .hover_style(|style| style.bg(rgba(palette.hover_surface)))
    };
    element
        .scale(if shown {
            metrics::SIDEBAR_TAB_SELECTED_SCALE
        } else {
            1.0
        })
        .transitions(|transitions| {
            transitions.scale(Duration::from_millis(
                metrics::SIDEBAR_TAB_SELECTION_DURATION_MS,
            ))
        })
}
