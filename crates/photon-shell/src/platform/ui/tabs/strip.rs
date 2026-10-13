//! The horizontal tab strip in the titlebar.

use gpui::{Role, ScrollHandle, div, prelude::*, px, rgb, rgba};

use super::super::controls::FocusRing;
use super::super::controls::OwnsClicks;
use super::super::icons::add_icon;
use super::super::layout::h_stack;
use super::super::{ClickHandler, metrics, theme::ThemeColors};
use super::{
    DraggedTab, TabDragAxis, TabDragMove, TabDrop, TabItem, TabParts, TabPrepaint,
    insertion_marker, is_foreign_tab_drag,
};

/// The tabs side by side, then a button that opens a new tab.
#[expect(
    clippy::too_many_arguments,
    reason = "passes the distinct scroll, drag, focus, and layout handles for this strip"
)]
pub(in super::super) fn tab_strip(
    tabs: Vec<TabItem>,
    on_new_tab: ClickHandler,
    on_drag_move: TabDragMove,
    on_drop: TabDrop,
    on_bounds: TabPrepaint,
    scroll_handle: ScrollHandle,
    palette: ThemeColors,
    elastic_scroll: bool,
) -> impl IntoElement {
    tab_strip_content(
        tabs,
        on_new_tab,
        on_drag_move,
        on_drop,
        on_bounds,
        scroll_handle,
        palette,
        elastic_scroll,
    )
    .w_full()
}

/// The intrinsic-width tab controls, without the full-width titlebar row.
/// Workspace edit mode uses this to frame the visible controls rather than
/// the empty titlebar around them.
#[expect(
    clippy::too_many_arguments,
    reason = "shares the tab-strip configuration with the full-width wrapper"
)]
pub(in super::super) fn tab_strip_content(
    tabs: Vec<TabItem>,
    on_new_tab: ClickHandler,
    on_drag_move: TabDragMove,
    on_drop: TabDrop,
    on_bounds: TabPrepaint,
    scroll_handle: ScrollHandle,
    palette: ThemeColors,
    elastic_scroll: bool,
) -> gpui::Div {
    let tab_count = tabs.len();
    let mut focus_index = 0;
    let mut strip = h_stack()
        .id("browser-tab-list")
        .role(Role::TabList)
        .aria_label("Browser tabs")
        .items_center()
        .gap(px(metrics::TAB_STRIP_GAP))
        .flex_initial()
        .min_w_0()
        .overflow_x_scroll()
        .elastic_scroll(elastic_scroll)
        .track_scroll(&scroll_handle);
    for (index, tab) in tabs.into_iter().enumerate() {
        let stops = TabParts::focus_stops(&tab.icon);
        let before = tab.insertion_before;
        let after = tab.insertion_after;
        if let Some(size) = before {
            strip = strip.child(insertion_marker(TabDragAxis::Horizontal, size, palette));
        }
        strip = strip.child(strip_tab(
            TabParts::new(
                tab,
                index,
                tab_count,
                focus_index,
                metrics::TAB_FAVICON_SIZE,
                palette,
                TabDragAxis::Horizontal,
            ),
            palette,
        ));
        if let Some(size) = after {
            strip = strip.child(insertion_marker(TabDragAxis::Horizontal, size, palette));
        }
        focus_index += stops;
    }
    let strip = strip
        .on_prepaint(move |prepaint, window, cx| {
            on_bounds(prepaint.bounds, window, cx);
        })
        .on_mouse_move_all(move |event, phase, hitbox, window, cx| {
            on_drag_move(event, phase, hitbox, window, cx);
        })
        .on_drag_over::<DraggedTab>(move |style, _, _, _| {
            style
                .border_1()
                .border_color(rgba(palette.soft_accent_border))
        })
        .can_drop(|value, window, _| is_foreign_tab_drag(value, window))
        .on_drop(move |dragged: &DraggedTab, window, cx| on_drop(dragged, window, cx));
    h_stack()
        .items_center()
        .gap(px(metrics::TAB_STRIP_GAP))
        .h(px(metrics::TITLEBAR_HEIGHT))
        .px(px(metrics::TAB_STRIP_INSET))
        .tab_group()
        .child(strip)
        .child(new_tab_button(on_new_tab, focus_index, palette))
}

/// A tab sized to share the strip: icon, title and close button. A pinned
/// tab is just its icon, at a fixed width.
fn strip_tab(parts: TabParts, palette: ThemeColors) -> impl IntoElement {
    if parts.pinned {
        let tab = parts
            .tab
            .flex_shrink_0()
            .justify_center()
            .w(px(metrics::PINNED_TAB_WIDTH))
            .h(px(metrics::TAB_HEIGHT))
            .rounded(px(metrics::CONTROL_RADIUS));
        let tab = if parts.active {
            tab.bg(rgba(palette.selected_surface))
        } else {
            tab.hover(|style| style.bg(rgba(palette.hover_surface)))
        };
        return tab.child(parts.icon);
    }
    let tab = parts
        .tab
        .flex_auto()
        .min_w(px(metrics::TAB_MIN_WIDTH))
        .max_w(px(metrics::TAB_MAX_WIDTH))
        .gap(px(metrics::TAB_CLOSE_GAP))
        .h(px(metrics::TAB_HEIGHT))
        .px(px(metrics::TAB_HORIZONTAL_PADDING))
        .rounded(px(metrics::CONTROL_RADIUS))
        .text_size(px(metrics::TAB_FONT_SIZE));
    let tab = if parts.active {
        tab.bg(rgba(palette.selected_surface))
    } else {
        tab.hover(|style| style.bg(rgba(palette.hover_surface)))
    };
    tab.child(parts.icon)
        .child(div().flex_1().min_w_0().truncate().child(parts.label))
        .child(parts.close)
}

fn new_tab_button(
    on_click: ClickHandler,
    focus_index: isize,
    palette: ThemeColors,
) -> impl IntoElement {
    h_stack()
        .id("browser-new-tab")
        .role(Role::Button)
        .aria_label("New tab")
        .tab_index(focus_index)
        .focus_ring(palette)
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(metrics::TAB_HEIGHT))
        .rounded(px(metrics::CONTROL_RADIUS))
        .text_color(rgb(palette.text_secondary))
        .hover(|style| style.bg(rgba(palette.hover_surface)))
        .owns_clicks()
        .on_click(on_click)
        .child(add_icon(palette.text_secondary, metrics::TAB_ICON_SIZE))
}
