//! Browser tab strip and its controls.

use gpui::{
    App, ClickEvent, FocusHandle, KeyDownEvent, MouseButton, Role, Window, div, prelude::*, px,
    rgb, rgba,
};

use super::icons::{add_icon, close_icon};
use super::layout::h_stack;
use super::theme::{Palette, metrics};

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

pub(super) struct TabItem {
    pub id: String,
    pub label: String,
    pub active: bool,
    pub focus_handle: FocusHandle,
    pub on_select: ClickHandler,
    pub on_key_down: Box<dyn Fn(&KeyDownEvent, &mut Window, &mut App)>,
    pub on_close: ClickHandler,
}

pub(super) fn tab_strip(
    tabs: Vec<TabItem>,
    on_new_tab: ClickHandler,
    palette: Palette,
) -> impl IntoElement {
    let tab_count = tabs.len();
    let mut tab_list = h_stack()
        .id("browser-tab-list")
        .role(Role::TabList)
        .aria_label("Browser tabs")
        .items_center()
        .gap(px(metrics::TAB_STRIP_GAP))
        .flex_initial()
        .min_w(px(0.0))
        .overflow_x_scroll();

    for (index, tab) in tabs.into_iter().enumerate() {
        tab_list = tab_list.child(browser_tab(
            tab,
            index + 1,
            tab_count,
            (index * 2) as isize,
            palette,
        ));
    }

    h_stack()
        .w_full()
        .items_center()
        .gap(px(metrics::TAB_STRIP_GAP))
        .h(px(metrics::TITLEBAR_HEIGHT))
        .px(px(metrics::TAB_STRIP_INSET))
        .tab_group()
        .child(tab_list)
        .child(new_tab_button(
            on_new_tab,
            (tab_count * 2) as isize,
            palette,
        ))
}

fn browser_tab(
    tab: TabItem,
    position: usize,
    tab_count: usize,
    focus_index: isize,
    palette: Palette,
) -> impl IntoElement {
    let close_label = format!("Close {}", tab.label);
    let close_id = format!("{}-close", tab.id);
    let focus_handle = tab.focus_handle.tab_index(focus_index).tab_stop(tab.active);
    let mut control = h_stack()
        .id(tab.id)
        .role(Role::Tab)
        .aria_label(tab.label.clone())
        .aria_selected(tab.active)
        .aria_position_in_set(position)
        .aria_size_of_set(tab_count)
        .track_focus(&focus_handle)
        .focus_visible(|style| style.border_1().border_color(rgb(palette.accent)))
        .flex_auto()
        .min_w(px(metrics::TAB_MIN_WIDTH))
        .max_w(px(metrics::TAB_MAX_WIDTH))
        .items_center()
        .gap(px(metrics::TAB_CLOSE_GAP))
        .h(px(metrics::TAB_HEIGHT))
        .px(px(metrics::TAB_HORIZONTAL_PADDING))
        .rounded(px(metrics::TAB_RADIUS))
        .text_size(px(metrics::TAB_FONT_SIZE))
        .text_color(rgb(if tab.active {
            palette.text
        } else {
            palette.text_muted
        }))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(tab.on_select)
        .on_key_down(tab.on_key_down);

    control = if tab.active {
        control.bg(rgba(palette.tab_active_surface))
    } else {
        control.hover(|style| style.bg(rgba(palette.tab_hover_surface)))
    };

    control
        .child(div().flex_1().min_w(px(0.0)).truncate().child(tab.label))
        .child(close_tab_button(
            close_id,
            close_label,
            tab.on_close,
            focus_index + 1,
            palette,
        ))
}

fn close_tab_button(
    id: String,
    label: String,
    on_click: ClickHandler,
    focus_index: isize,
    palette: Palette,
) -> impl IntoElement {
    h_stack()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .tab_index(focus_index)
        .focus_visible(|style| style.border_1().border_color(rgb(palette.accent)))
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(metrics::TAB_CLOSE_BUTTON_SIZE))
        .rounded(px(metrics::TAB_RADIUS))
        .text_color(rgb(palette.text_muted))
        .hover(|style| style.bg(rgba(palette.tab_control_hover)))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(on_click)
        .child(close_icon(palette.text_muted, metrics::TAB_ICON_SIZE))
}

fn new_tab_button(
    on_click: ClickHandler,
    focus_index: isize,
    palette: Palette,
) -> impl IntoElement {
    h_stack()
        .id("browser-new-tab")
        .role(Role::Button)
        .aria_label("New tab")
        .tab_index(focus_index)
        .focus_visible(|style| style.border_1().border_color(rgb(palette.accent)))
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(metrics::TAB_HEIGHT))
        .rounded(px(metrics::TAB_RADIUS))
        .text_color(rgb(palette.text_muted))
        .hover(|style| style.bg(rgba(palette.tab_control_hover)))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(on_click)
        .child(add_icon(palette.text_muted, metrics::TAB_ICON_SIZE))
}
