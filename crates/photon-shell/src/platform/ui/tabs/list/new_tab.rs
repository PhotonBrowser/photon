//! The sidebar's new-tab action in expanded and collapsed layouts.

use gpui::{App, Window, prelude::*, px, rgb, rgba};

use super::super::super::animated_icons::{animated_icon, plus};
use super::super::super::controls::FocusRing;
use super::super::super::controls::{OwnsClicks, Pressable};
use super::super::super::layout::h_stack;
use super::super::super::motion::SurfaceFade;
use super::super::super::sidebar::collapsed_tile;
use super::super::super::tooltip::label_tooltip;
use super::super::super::{ClickHandler, metrics, theme::ThemeColors};

/// “New Tab”, or only the plus icon when the sidebar is collapsed.
pub(super) fn new_tab_row(
    on_click: ClickHandler,
    collapsed: bool,
    suppress_tooltip: bool,
    focus_index: isize,
    palette: ThemeColors,
    window: &mut Window,
    cx: &mut App,
) -> gpui_motion::PressScaleElement<gpui::Stateful<gpui::Div>> {
    let row = h_stack()
        .id("browser-new-tab")
        .role(gpui::Role::Button)
        .aria_label("New tab")
        .tab_index(focus_index)
        .focus_ring(palette)
        .items_center()
        .rounded(px(metrics::sidebar_item_radius()))
        .text_size(px(metrics::SIDEBAR_FONT_SIZE))
        .text_color(rgb(palette.text_secondary))
        .surface_fade()
        .hover_style(|style| style.bg(rgba(palette.hover_surface)))
        .owns_clicks()
        .pressable()
        .on_click(on_click)
        .relative()
        .child(animated_icon(
            "browser-new-tab",
            plus(),
            palette.text_secondary,
            metrics::SidebarSpacing::new(collapsed).icon_size,
            window,
            cx,
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
