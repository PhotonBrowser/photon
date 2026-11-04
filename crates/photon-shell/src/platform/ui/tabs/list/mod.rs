//! The sidebar's vertical tab list: pinned tabs, then today's tabs under a
//! row that opens a new tab. Collapsed to icons, each is a square tile.

use gpui::{App, Role, Window, div, prelude::*, px, rgba};

use super::super::layout::v_stack;
use super::super::motion::ReorderGroup;
use super::super::{ClickHandler, metrics, theme::ThemeColors};
use super::{
    DraggedTab, TabDragAxis, TabDragMove, TabDrop, TabItem, TabParts, TabPrepaint,
    insertion_marker, is_foreign_tab_drag, reorder_tab_items, sync_reorder_order,
};
use std::time::Duration;

mod new_tab;
mod tab_item;

use new_tab::new_tab_row;
use tab_item::{split_group, tab_row, tab_tile};

/// The tabs, one row each: pinned tabs above a divider, then today's tabs
/// and a row that opens a new tab. Collapsed, each row is only its icon. The
/// result borrows nothing, so the caller can keep using `cx`.
#[expect(
    clippy::too_many_arguments,
    reason = "passes the distinct drag, focus, and layout callbacks for this list"
)]
pub(in super::super) fn tab_list(
    tabs: Vec<TabItem>,
    on_new_tab: ClickHandler,
    on_drop_into_today: TabDrop,
    on_drag_move: TabDragMove,
    on_drop: TabDrop,
    on_bounds: TabPrepaint,
    collapsed: bool,
    new_tab_at_end: bool,
    new_tab_button_motion: Option<(String, f32, Duration)>,
    palette: ThemeColors,
    pinned_reorder: ReorderGroup<u64>,
    today_reorder: ReorderGroup<u64>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement + use<> {
    let spacing = metrics::SidebarSpacing::new(collapsed);
    let suppress_tooltips = tabs.iter().any(|tab| tab.suppress_tooltips);
    let has_live_pinned = tabs.iter().any(|tab| tab.pinned);
    sync_reorder_order(&pinned_reorder, &tabs, true);
    sync_reorder_order(&today_reorder, &tabs, false);
    let tabs = reorder_tab_items(tabs, &pinned_reorder, &today_reorder);
    let tab_count = tabs.len();
    let mut focus_index = if new_tab_at_end { 0 } else { 1 };
    let (mut pinned, mut today) = (Vec::new(), Vec::new());
    let mut slots: Vec<Option<TabItem>> = tabs.into_iter().map(Some).collect();
    for index in 0..slots.len() {
        let Some(tab) = slots[index].take() else {
            continue;
        };
        // A split's second tab joins the first's row, wherever it is listed.
        let partner = tab.split_partner.and_then(|partner| {
            let at = slots.iter().position(|slot| {
                slot.as_ref()
                    .is_some_and(|other| other.reorder_key == partner && other.pinned == tab.pinned)
            })?;
            Some((at, slots[at].take()?))
        });
        let is_pinned = tab.pinned;
        let section = if is_pinned { &mut pinned } else { &mut today };
        let reorder = if is_pinned {
            &pinned_reorder
        } else {
            &today_reorder
        };
        let before = tab.insertion_before;
        let after = partner
            .as_ref()
            .map_or(tab.insertion_after, |(_, partner)| partner.insertion_after);
        if let Some(size) = before {
            section.push(insertion_marker(TabDragAxis::Vertical, size, palette));
        }
        let mut item = |tab: TabItem, position: usize| {
            let stops = TabParts::focus_stops(&tab.icon);
            let parts = TabParts::new(
                tab,
                position,
                tab_count,
                focus_index,
                spacing.icon_size,
                palette,
                TabDragAxis::Vertical,
            );
            focus_index += stops;
            if collapsed {
                tab_tile(parts, palette, reorder)
            } else {
                tab_row(parts, palette, reorder)
            }
        };
        let first = item(tab, index);
        section.push(match partner {
            Some((at, partner)) => split_group(first, item(partner, at), collapsed, palette),
            None => first,
        });
        if let Some(size) = after {
            section.push(insertion_marker(TabDragAxis::Vertical, size, palette));
        }
    }
    let section = |id: &'static str, label: &'static str, rows: Vec<_>, close_empty_slot: bool| {
        v_stack()
            .id(id)
            .role(Role::TabList)
            .aria_label(label)
            .w_full()
            .gap(px(spacing.item_gap))
            .when(close_empty_slot, |section| {
                section.mb(px(-spacing.item_gap))
            })
            .children(rows)
    };
    let on_drop_into_today = std::rc::Rc::clone(&on_drop_into_today);
    let divider = has_live_pinned.then(|| {
        let on_drop = std::rc::Rc::clone(&on_drop_into_today);
        div()
            .id("browser-tab-divider")
            .w_full()
            // The list's gaps would add to the divider's own margin, so it
            // takes them back: a separator is the same height everywhere,
            // while its margin still catches a drop.
            .my(px(-spacing.item_gap))
            .py(px(spacing.separator_margin))
            .px(px(if collapsed {
                spacing.padding
            } else {
                metrics::SIDEBAR_TAB_PADDING
            }))
            .rounded(px(metrics::sidebar_item_radius()))
            .on_drag_over::<DraggedTab>(move |style, _, _, _| style.bg(rgba(palette.hover_surface)))
            .on_drop(move |dragged: &DraggedTab, window, cx| on_drop(dragged, window, cx))
            .child(
                div()
                    .h(px(metrics::MENU_SEPARATOR_HEIGHT))
                    .bg(rgba(palette.menu_border)),
            )
    });
    let new_tab = new_tab_row(
        on_new_tab,
        collapsed,
        suppress_tooltips,
        if new_tab_at_end { focus_index } else { 0 },
        palette,
        window,
        cx,
    )
    .on_drag_over::<DraggedTab>(move |style, _, _, _| style.bg(rgba(palette.hover_surface)))
    .on_drop(move |dragged: &DraggedTab, window, cx| on_drop_into_today(dragged, window, cx));
    let new_tab = match new_tab_button_motion {
        Some((id, from, duration)) => {
            super::super::motion::animate_vertical_position(new_tab, id, from, duration)
        }
        None => new_tab.into_any_element(),
    };
    // With no tabs of today, the empty list would add a gap of its own.
    let today =
        (!today.is_empty()).then(|| section("browser-tab-list", "Browser tabs", today, false));
    let list = v_stack()
        .id("sidebar-tab-list-drag-target")
        .w_full()
        // Keep the drop target viewport-sized when it fits, and let overflowing rows grow it.
        .min_h_full()
        .flex_shrink_0()
        .gap(px(spacing.item_gap))
        .tab_group()
        .when(!pinned.is_empty(), |list| {
            list.child(section(
                "browser-pinned-tabs",
                "Pinned tabs",
                pinned,
                !has_live_pinned,
            ))
        })
        .children(divider);
    let list = if new_tab_at_end {
        list.children(today).child(new_tab)
    } else {
        list.child(new_tab).children(today)
    };
    list.on_mouse_move_all(move |event, phase, hitbox, window, cx| {
        on_drag_move(event, phase, hitbox, window, cx);
    })
    .on_prepaint(move |prepaint, window, cx| {
        on_bounds(prepaint.bounds, window, cx);
    })
    .on_drag_over::<DraggedTab>(move |style, _, _, _| {
        style
            .border_1()
            .border_color(rgba(palette.soft_accent_border))
    })
    .can_drop(|value, window, _| is_foreign_tab_drag(value, window))
    .on_drop(move |dragged: &DraggedTab, window, cx| on_drop(dragged, window, cx))
}
