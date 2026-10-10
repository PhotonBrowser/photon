//! The sidebar's vertical tab list: pinned tabs, then today's tabs under a
//! row that opens a new tab.

use gpui::{MouseButton, Role, div, prelude::*, px, rgb, rgba};

use super::super::icons::add_icon;
use super::super::layout::{h_stack, v_stack};
use super::super::{ClickHandler, metrics, theme::ThemeColors};
use super::{DraggedTab, TabDrop, TabItem, TabParts};

/// The tabs, one row each: pinned tabs above a divider, then a row that
/// opens a new tab and today's tabs. A tab dropped on the divider or that
/// row joins today's tabs.
pub(in super::super) fn tab_list(
    tabs: Vec<TabItem>,
    on_new_tab: ClickHandler,
    on_drop_into_today: TabDrop,
    palette: ThemeColors,
) -> impl IntoElement {
    let tab_count = tabs.len();
    let mut focus_index = 1;
    let (mut pinned, mut today) = (Vec::new(), Vec::new());
    for (index, tab) in tabs.into_iter().enumerate() {
        let stops = TabParts::focus_stops(&tab.icon);
        let section = if tab.pinned { &mut pinned } else { &mut today };
        section.push(tab_row(
            TabParts::new(
                tab,
                index,
                tab_count,
                focus_index,
                metrics::SIDEBAR_ICON_SIZE,
                palette,
            ),
            palette,
        ));
        focus_index += stops;
    }
    let section = |id: &'static str, label: &'static str, rows: Vec<_>| {
        v_stack()
            .id(id)
            .role(Role::TabList)
            .aria_label(label)
            .w_full()
            .gap(px(metrics::SIDEBAR_ITEM_GAP))
            .children(rows)
    };
    let on_drop_into_today = std::rc::Rc::new(on_drop_into_today);
    let divider = (!pinned.is_empty()).then(|| {
        let on_drop = on_drop_into_today.clone();
        div()
            .id("browser-tab-divider")
            .w_full()
            .py(px(metrics::SIDEBAR_SEPARATOR_MARGIN))
            .px(px(metrics::SIDEBAR_TAB_PADDING))
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
        .w_full()
        .gap(px(metrics::SIDEBAR_ITEM_GAP))
        .tab_group()
        .when(!pinned.is_empty(), |list| {
            list.child(section("browser-pinned-tabs", "Pinned tabs", pinned))
        })
        .children(divider)
        .child(
            new_tab_row(on_new_tab, palette)
                .drag_over::<DraggedTab>(move |style, _, _, _| {
                    style.bg(rgba(palette.hover_surface))
                })
                .on_drop(move |dragged: &DraggedTab, window, cx| {
                    on_drop_into_today(dragged, window, cx)
                }),
        )
        .child(section("browser-tab-list", "Browser tabs", today))
}

/// A full-width row: icon, title, and a close button on the active tab and
/// on the row under the pointer. Hidden, the close button still sits under
/// the pointer only when its row is hovered, so it is never clicked unseen.
fn tab_row(parts: TabParts, palette: ThemeColors) -> impl IntoElement {
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
        row.bg(rgba(palette.surface))
    } else {
        row.hover(|style| style.bg(rgba(palette.hover_surface)))
    };
    let close = if parts.active {
        parts.close
    } else {
        parts
            .close
            .opacity(0.0)
            .group_hover(parts.group, |style| style.opacity(1.0))
    };
    row.child(parts.icon)
        .child(div().flex_1().min_w_0().truncate().child(parts.label))
        .child(close)
}

/// "+ New Tab", above the tabs.
fn new_tab_row(on_click: ClickHandler, palette: ThemeColors) -> gpui::Stateful<gpui::Div> {
    h_stack()
        .id("browser-new-tab")
        .role(Role::Button)
        .aria_label("New tab")
        .tab_index(0)
        .focus_visible(|style| style.border_1().border_color(rgb(palette.chosen)))
        .w_full()
        .items_center()
        .gap(px(metrics::SIDEBAR_TAB_ICON_GAP))
        .h(px(metrics::SIDEBAR_TAB_HEIGHT))
        .px(px(metrics::SIDEBAR_TAB_PADDING))
        .rounded(px(metrics::SIDEBAR_ITEM_RADIUS))
        .text_size(px(metrics::SIDEBAR_FONT_SIZE))
        .text_color(rgb(palette.text_secondary))
        .hover(|style| style.bg(rgba(palette.hover_surface)))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(on_click)
        .child(add_icon(palette.text_secondary, metrics::SIDEBAR_ICON_SIZE))
        .child("New Tab")
}
