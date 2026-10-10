//! Page layout shared by Photon's own pages: columns, headings and groups.

use gpui::{FontWeight, Role, SharedString, div, prelude::*, px, rgb, rgba};

use super::super::layout::v_stack;
use super::super::{metrics, theme::ThemeColors};

/// A scrolling column of content, centered in the space it is given.
pub(super) fn scrolling_column(
    id: impl Into<gpui::ElementId>,
    content: impl IntoElement,
) -> impl IntoElement {
    v_stack()
        .id(id)
        .size_full()
        .items_center()
        .overflow_y_scroll()
        .p(px(metrics::INTERNAL_PAGE_INSET))
        .child(
            v_stack()
                .w_full()
                .max_w(px(metrics::INTERNAL_PAGE_MAX_WIDTH))
                .child(content),
        )
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

/// A label above a control, such as "Shortcuts" above its choices.
pub(super) fn label(text: &'static str, palette: ThemeColors) -> impl IntoElement {
    div()
        .text_size(px(metrics::TAB_FONT_SIZE))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(palette.text_secondary))
        .child(text)
}

pub(super) fn secondary_text(
    text: impl Into<SharedString>,
    palette: ThemeColors,
) -> impl IntoElement {
    div()
        .text_size(px(metrics::TAB_FONT_SIZE))
        .text_color(rgb(palette.text_secondary))
        .child(text.into())
}

/// A panel holding related rows.
pub(super) fn group(palette: ThemeColors) -> gpui::Div {
    v_stack()
        .w_full()
        .p(px(metrics::MENU_PADDING))
        .gap(px(metrics::MENU_ITEM_GAP))
        .rounded(px(metrics::SURFACE_RADIUS))
        .border_1()
        .border_color(rgba(palette.menu_border))
        .bg(rgba(palette.menu_surface))
}
