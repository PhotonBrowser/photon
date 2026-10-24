//! Shared GPUI image renderer for the portable Engine frame path.

use gpui::{AnyElement, ImageSource, ObjectFit, img, prelude::*, px};

use crate::platform::{PageSurfacePalette, cpu_presentation::PresentedSurface, metrics};

pub(in crate::platform) fn replace_surface(
    _: Option<PresentedSurface>,
    next: PresentedSurface,
) -> PresentedSurface {
    next
}

pub(in crate::platform) fn page_surface(
    presented: Option<&PresentedSurface>,
    palette: PageSurfacePalette,
) -> AnyElement {
    let mut page = gpui::div()
        .size_full()
        .rounded(px(metrics::page_radius()))
        .rounded_smoothing(metrics::page_smoothing())
        .bg(gpui::rgb(palette.background));
    if let Some(presented) = presented {
        page = page.child(
            img(ImageSource::Render(presented.image.clone()))
                .size_full()
                .object_fit(ObjectFit::Fill),
        );
    }
    page.into_any_element()
}
