//! The horizontal tab strip in the titlebar.

use gpui::{MouseButton, Role, div, prelude::*, px, rgb, rgba};

use super::super::icons::add_icon;
use super::super::layout::h_stack;
use super::super::{ClickHandler, metrics, theme::ThemeColors};
use super::{TabItem, TabParts};

/// The tabs side by side, then a button that opens a new tab.
pub(in super::super) fn tab_strip(
    tabs: Vec<TabItem>,
    on_new_tab: ClickHandler,
    palette: ThemeColors,
) -> impl IntoElement {
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
        .overflow_x_scroll();
    for (index, tab) in tabs.into_iter().enumerate() {
        let stops = TabParts::focus_stops(&tab.icon);
        strip = strip.child(strip_tab(
            TabParts::new(
                tab,
                index,
                tab_count,
                focus_index,
                metrics::TAB_FAVICON_SIZE,
                palette,
            ),
            palette,
        ));
        focus_index += stops;
    }
    h_stack()
        .w_full()
        .items_center()
        .gap(px(metrics::TAB_STRIP_GAP))
        .h(px(metrics::TITLEBAR_HEIGHT))
        .px(px(metrics::TAB_STRIP_INSET))
        .tab_group()
        .child(strip)
        .child(new_tab_button(on_new_tab, focus_index, palette))
}

/// A tab sized to share the strip: icon, title and close button.
fn strip_tab(parts: TabParts, palette: ThemeColors) -> impl IntoElement {
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
        tab.bg(rgba(palette.surface))
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
        .focus_visible(|style| style.border_1().border_color(rgb(palette.chosen)))
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(metrics::TAB_HEIGHT))
        .rounded(px(metrics::CONTROL_RADIUS))
        .text_color(rgb(palette.text_secondary))
        .hover(|style| style.bg(rgba(palette.hover_surface)))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(on_click)
        .child(add_icon(palette.text_secondary, metrics::TAB_ICON_SIZE))
}
