//! macOS page renderer: Core Animation shows Engine's IOSurface in a layer
//! beneath GPUI's, and GPUI leaves a hole where the page is.

use gpui::{AnyElement, prelude::*, px};

use crate::platform::{PagePlacement, PageSurfacePalette, metrics};

use super::page_layer::PageLayer;
use super::presentation::PresentationRuntime;
pub(in crate::platform) use super::presentation::PresentedSurface;

/// Shows a web view's frames in its own Core Animation layer.
pub(in crate::platform) struct PagePresenter {
    layer: PageLayer,
}

impl PagePresenter {
    pub(in crate::platform) fn new() -> Self {
        Self {
            layer: PageLayer::new(),
        }
    }

    /// Shows `presented` and releases the frames it replaced. Returns whether
    /// it is on screen already; here it is, and GPUI has nothing to draw.
    pub(in crate::platform) fn show(
        &mut self,
        presented: &mut PresentedSurface,
        runtime: &PresentationRuntime,
    ) -> bool {
        let identity = (
            presented.key.backing,
            presented.key.generation,
            presented.key.frame,
        );
        if !self.layer.shows(identity) {
            self.layer.set_surface(
                identity,
                &presented.iosurface,
                presented.visible_width,
                presented.visible_height,
            );
            runtime.present(presented);
        }
        true
    }

    pub(in crate::platform) fn place(&mut self, window: &gpui::Window, placement: PagePlacement) {
        self.layer.place(window, placement);
    }

    pub(in crate::platform) fn hide(&mut self) {
        self.layer.hide();
    }

    pub(in crate::platform) fn detach(&mut self) {
        self.layer.detach();
    }
}

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
