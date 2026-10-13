//! The sidebar's vertical tab list: pinned tabs, then today's tabs under a
//! row that opens a new tab. Collapsed to icons, each is a square tile.

use gpui::{AnyElement, Role, div, prelude::*, px, rgb, rgba};

use super::super::controls::FocusRing;
use super::super::controls::{OwnsClicks, Pressable};
use super::super::icons::add_icon;
use super::super::layout::{h_stack, v_stack};
use super::super::sidebar::collapsed_tile;
use super::super::tooltip::label_tooltip;
use super::super::{ClickHandler, metrics, theme::ThemeColors};
use super::{
    DraggedTab, TabDragAxis, TabDragMove, TabDrop, TabItem, TabParts, TabPrepaint,
    insertion_marker, is_foreign_tab_drag,
};

/// The tabs, one row each: pinned tabs above a divider, then a row that
/// opens a new tab and today's tabs. Collapsed, each row is only its icon.
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
    palette: ThemeColors,
) -> impl IntoElement {
    let spacing = metrics::SidebarSpacing::new(collapsed);
    let suppress_tooltips = tabs.iter().any(|tab| tab.suppress_tooltips);
    let layout_tab: fn(TabParts, ThemeColors) -> AnyElement =
        if collapsed { tab_tile } else { tab_row };
    let tab_count = tabs.len();
    let mut focus_index = 1;
    let (mut pinned, mut today) = (Vec::new(), Vec::new());
    for (index, tab) in tabs.into_iter().enumerate() {
        let stops = TabParts::focus_stops(&tab.icon);
        let section = if tab.pinned { &mut pinned } else { &mut today };
        let before = tab.insertion_before;
        let after = tab.insertion_after;
        if let Some(size) = before {
            section.push(insertion_marker(TabDragAxis::Vertical, size, palette));
        }
        section.push(layout_tab(
            TabParts::new(
                tab,
                index,
                tab_count,
                focus_index,
                spacing.icon_size,
                palette,
                TabDragAxis::Vertical,
            ),
            palette,
        ));
        if let Some(size) = after {
            section.push(insertion_marker(TabDragAxis::Vertical, size, palette));
        }
        focus_index += stops;
    }
    let section = |id: &'static str, label: &'static str, rows: Vec<_>| {
        v_stack()
            .id(id)
            .role(Role::TabList)
            .aria_label(label)
            .w_full()
            .gap(px(spacing.item_gap))
            .children(rows)
    };
    let on_drop_into_today = std::rc::Rc::clone(&on_drop_into_today);
    let divider = (!pinned.is_empty()).then(|| {
        let on_drop = std::rc::Rc::clone(&on_drop_into_today);
        div()
            .id("browser-tab-divider")
            .w_full()
            .py(px(spacing.separator_margin))
            .px(px(if collapsed {
                spacing.padding
            } else {
                metrics::SIDEBAR_TAB_PADDING
            }))
            .rounded(px(metrics::SIDEBAR_ITEM_RADIUS))
            .drag_over::<DraggedTab>(move |style, _, _, _| style.bg(rgba(palette.hover_surface)))
            .on_drop(move |dragged: &DraggedTab, window, cx| on_drop(dragged, window, cx))
            .child(
                div()
                    .h(px(metrics::MENU_SEPARATOR_HEIGHT))
                    .bg(rgba(palette.menu_border)),
            )
    });
    v_stack()
        .id("sidebar-tab-list-drag-target")
        .w_full()
        .h_full()
        .gap(px(spacing.item_gap))
        .tab_group()
        .when(!pinned.is_empty(), |list| {
            list.child(section("browser-pinned-tabs", "Pinned tabs", pinned))
        })
        .children(divider)
        .child(
            new_tab_row(on_new_tab, collapsed, suppress_tooltips, palette)
                .drag_over::<DraggedTab>(move |style, _, _, _| {
                    style.bg(rgba(palette.hover_surface))
                })
                .on_drop(move |dragged: &DraggedTab, window, cx| {
                    on_drop_into_today(dragged, window, cx)
                }),
        )
        .child(section("browser-tab-list", "Browser tabs", today))
        .on_mouse_move_all(move |event, phase, hitbox, window, cx| {
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

/// A full-width row: icon, title, and a close button on the active tab and
/// on the row under the pointer. Hidden, the close button still sits under
/// the pointer only when its row is hovered, so it is never clicked unseen.
fn tab_row(parts: TabParts, palette: ThemeColors) -> AnyElement {
    let row = parts
        .tab
        .w_full()
        .gap(px(metrics::SIDEBAR_TAB_ICON_GAP))
        .h(px(metrics::SIDEBAR_TAB_HEIGHT))
        .pl(px(metrics::SIDEBAR_TAB_PADDING))
        .pr(px(metrics::SIDEBAR_PADDING))
        .rounded(px(metrics::SIDEBAR_ITEM_RADIUS))
        .text_size(px(metrics::SIDEBAR_FONT_SIZE));
    let row = if parts.active {
        row.bg(rgba(palette.selected_surface))
    } else {
        row.hover(|style| style.bg(rgba(palette.hover_surface)))
    };
    let close = if parts.active {
        parts.close
    } else {
        // Hidden, it is also left out of the tab order, so keyboard focus
        // never lands on it unseen.
        parts
            .close
            .tab_stop(false)
            .opacity(0.0)
            .group_hover(parts.group, |style| style.opacity(1.0))
    };
    row.child(parts.icon)
        .child(div().flex_1().min_w_0().truncate().child(parts.label))
        .child(close)
        .into_any_element()
}

/// A square holding only the tab's icon, for the collapsed sidebar. The
/// tab's title stays its accessible name; middle-click still closes it.
fn tab_tile(parts: TabParts, palette: ThemeColors) -> AnyElement {
    let tooltip = (!parts.suppress_tooltips).then(|| label_tooltip(parts.label.clone(), palette));
    let tile = collapsed_tile(parts.tab)
        .relative()
        .justify_center()
        .when_some(tooltip, |tile, tooltip| tile.tooltip(tooltip));
    let tile = if parts.active {
        tile.bg(rgba(palette.selected_surface))
    } else {
        tile.hover(|style| style.bg(rgba(palette.hover_surface)))
    };
    tile.child(parts.icon).into_any_element()
}

/// "+ New Tab", above the tabs; collapsed, only "+".
fn new_tab_row(
    on_click: ClickHandler,
    collapsed: bool,
    suppress_tooltip: bool,
    palette: ThemeColors,
) -> gpui::Stateful<gpui::Div> {
    let row = h_stack()
        .id("browser-new-tab")
        .role(Role::Button)
        .aria_label("New tab")
        .tab_index(0)
        .focus_ring(palette)
        .items_center()
        .rounded(px(metrics::SIDEBAR_ITEM_RADIUS))
        .text_size(px(metrics::SIDEBAR_FONT_SIZE))
        .text_color(rgb(palette.text_secondary))
        .hover(|style| style.bg(rgba(palette.hover_surface)))
        .owns_clicks()
        .pressable()
        .on_click(on_click)
        .child(add_icon(
            palette.text_secondary,
            metrics::SidebarSpacing::new(collapsed).icon_size,
        ));
    if collapsed {
        let row = collapsed_tile(row).justify_center();
        return if suppress_tooltip {
            row
        } else {
            row.tooltip(label_tooltip("New tab", palette))
        };
    }
    row.w_full()
        .gap(px(metrics::SIDEBAR_TAB_ICON_GAP))
        .h(px(metrics::SIDEBAR_TAB_HEIGHT))
        .px(px(metrics::SIDEBAR_TAB_PADDING))
        .child("New tab")
}
