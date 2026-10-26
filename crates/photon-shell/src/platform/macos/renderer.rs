//! macOS page renderer: Core Animation shows Engine's IOSurface in a layer
//! beneath GPUI's, and GPUI leaves a hole where the page is.

use gpui::{AnyElement, prelude::*, px};

use crate::platform::{PageSurfacePalette, metrics};

pub(in crate::platform) use super::presentation::PresentedSurface;

/// Makes `next` the page's frame. The frames it replaced are released once
/// it is displayed.
pub(in crate::platform) fn replace_surface(
    previous: Option<PresentedSurface>,
    mut next: PresentedSurface,
) -> PresentedSurface {
    if let Some(previous) = previous {
        next.replaced.extend(previous.into_release_keys());
    }
    next
}

/// The page's place in GPUI: a hole onto its Core Animation layer. Whatever
/// is drawn after it, over the page, stays on top.
pub(in crate::platform) fn page_surface(
    _: Option<&PresentedSurface>,
    _: PageSurfacePalette,
) -> AnyElement {
    gpui::canvas(
        |_, _, _| {},
        |bounds, _, window, _| {
            window.paint_cutout(
                bounds,
                px(metrics::page_radius()).into(),
                metrics::page_smoothing(),
            )
        },
    )
    .size_full()
    .into_any_element()
}
