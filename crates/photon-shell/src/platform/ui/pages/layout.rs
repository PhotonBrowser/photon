//! Page layout shared by Photon's own pages: columns, headings and groups.

use gpui::{FontWeight, Role, SharedString, div, prelude::*, px, rgb, rgba};

use super::super::layout::v_stack;
use super::super::{metrics, theme::ThemeColors};

pub(super) use super::super::layout::{label, secondary_text};

/// A scrolling column of content, centered in the space it is given.
pub(super) fn scrolling_column(
    id: impl Into<gpui::ElementId>,
    content: impl IntoElement,
    elastic_scroll: bool,
    max_width: f32,
) -> impl IntoElement {
    v_stack()
        .id(id)
        .size_full()
        .items_center()
        .overflow_y_scroll()
        .elastic_scroll(elastic_scroll)
        .p(px(metrics::INTERNAL_PAGE_INSET))
        .child(v_stack().w_full().max_w(px(max_width)).child(content))
}

/// A heading with a line of explanation under it.
pub(super) fn heading(
    id: &'static str,
    title: &'static str,
    description: impl Into<SharedString>,
    palette: ThemeColors,
) -> impl IntoElement {
    v_stack()
        .gap(px(metrics::MENU_ITEM_GAP))
        .child(
            div()
                .id(id)
                .role(Role::Heading)
                .aria_level(2)
                .text_size(px(metrics::INTERNAL_PAGE_SECTION_SIZE))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(palette.text_primary))
                .child(title),
        )
        .child(secondary_text(description, palette))
}

/// A panel holding related rows.
pub(super) fn group(palette: ThemeColors) -> gpui::Div {
    v_stack()
        .w_full()
        .p(px(metrics::MENU_PADDING))
        .gap(px(metrics::MENU_ITEM_GAP))
        .rounded(px(metrics::surface_radius()))
        .border_1()
        .border_color(rgba(palette.menu_border))
        .bg(rgba(palette.menu_surface))
}

/// A group that holds a segmented choice control, with corners derived from
/// the control's inset and border.
pub(super) fn group_with_choices(palette: ThemeColors) -> gpui::Div {
    group(palette).rounded(px(metrics::choices_group_radius()))
}
