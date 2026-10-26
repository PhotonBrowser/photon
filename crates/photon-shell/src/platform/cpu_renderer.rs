//! Shared GPUI image renderer for the portable Engine frame path.

use gpui::{AnyElement, ImageSource, ObjectFit, img, prelude::*, px};

use crate::platform::{
    PagePlacement, PageSurfacePalette,
    cpu_presentation::{PresentationRuntime, PresentedSurface},
    metrics,
};

/// Shows a web view's frames by drawing them with GPUI, from pixels the
/// Engine copied. A platform that composites Engine surfaces natively, as
/// macOS does with Core Animation, implements this same interface in its own
/// `renderer` module and leaves `page_surface` a hole onto its layer.
pub(in crate::platform) struct PagePresenter;

impl PagePresenter {
    pub(in crate::platform) fn new() -> Self {
        Self
    }

    /// Returns whether `presented` is on screen already; here GPUI must
    /// redraw to show it.
    pub(in crate::platform) fn show(
        &mut self,
        _: &mut PresentedSurface,
        _: &PresentationRuntime,
    ) -> bool {
        false
    }

    /// GPUI lays the page out itself; there is no native layer to place.
    pub(in crate::platform) fn place(&mut self, _: &gpui::Window, _: PagePlacement) {}

    pub(in crate::platform) fn hide(&mut self) {}

    pub(in crate::platform) fn detach(&mut self) {}
}

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
