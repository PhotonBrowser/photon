//! Linux page renderer placeholder.
//!
//! This is the target-specific page layer used by the shared WebView. It stays
//! empty until the Linux Engine surface backend is connected.

use gpui::{AnyElement, Role, div, prelude::*, px, rgb, rgba};

use crate::platform::{PageSurfacePalette, metrics};

pub(crate) struct PresentedSurface;

pub(crate) fn replace_surface(
    _: Option<PresentedSurface>,
    next: PresentedSurface,
) -> PresentedSurface {
    next
}

pub(crate) fn page_surface(
    _: Option<&PresentedSurface>,
    palette: PageSurfacePalette,
) -> AnyElement {
    div()
        .size_full()
        .relative()
        .rounded(px(metrics::PAGE_RADIUS))
        .bg(rgb(palette.background))
        .child(
            div()
                .absolute()
                .right(px(metrics::CHIP_INSET))
                .bottom(px(metrics::CHIP_INSET))
                .role(Role::Status)
                .aria_label("Linux renderer not implemented yet")
                .px(px(metrics::CHIP_PADDING))
                .py(px((metrics::CHIP_HEIGHT - metrics::CHIP_FONT_SIZE) / 2.0))
                .rounded(px(metrics::CHIP_RADIUS))
                .border_1()
                .border_color(rgba(palette.notice_border))
                .bg(rgb(palette.notice_surface))
                .text_size(px(metrics::CHIP_FONT_SIZE))
                .text_color(rgb(palette.notice_text))
                .child("Linux renderer not implemented yet"),
        )
        .into_any_element()
}
