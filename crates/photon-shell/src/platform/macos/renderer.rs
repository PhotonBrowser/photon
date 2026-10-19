//! macOS page renderer: Engine frames arrive as externally presented Metal surfaces.

use gpui::{AnyElement, ObjectFit, SurfaceSource, prelude::*, px, surface};

use crate::platform::{PageSurfacePalette, metrics};

pub(in crate::platform) use super::presentation::PresentedSurface;

pub(in crate::platform) fn replace_surface(
    previous: Option<PresentedSurface>,
    next: PresentedSurface,
) -> PresentedSurface {
    if let Some(previous) = previous {
        let pending = std::mem::take(&mut *previous.retired_until_submitted.lock().unwrap());
        let mut retired = next.retired_until_submitted.lock().unwrap();
        retired.push(previous.retire());
        retired.extend(pending);
    }
    next
}

pub(in crate::platform) fn page_surface(
    presented: Option<&PresentedSurface>,
    palette: PageSurfacePalette,
) -> AnyElement {
    let mut page = gpui::div()
        .size_full()
        .rounded(px(metrics::page_radius()))
        .bg(gpui::rgb(palette.background));
    if let Some(presented) = presented {
        page = page.child(
            surface(SurfaceSource::ExternalMetal(presented.surface.clone()))
                .size_full()
                .object_fit(ObjectFit::Fill)
                .relative()
                .rounded(px(metrics::page_radius())),
        );
    }
    page.into_any_element()
}
