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
    /// Creates a presenter with its own Core Animation page layer.
    pub(in crate::platform) fn new() -> Self {
        Self {
            layer: PageLayer::new(),
        }
    }

    /// Installs `presented` and releases the frames it replaced. Returns
    /// whether it is on screen already; here it is, and GPUI has nothing to
    /// draw.
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

    /// Places the native layer beneath GPUI's page cutout.
    pub(in crate::platform) fn place(&mut self, window: &gpui::Window, placement: PagePlacement) {
        self.layer.place(window, placement);
    }

    /// Hides this page's native layer while its tab or window is hidden.
    pub(in crate::platform) fn hide(&mut self) {
        self.layer.hide();
    }

    /// Removes this page's native layer from the GPUI window during shutdown.
    pub(in crate::platform) fn detach(&mut self) {
        self.layer.detach();
    }
}

/// Makes `next` the page's frame, retaining leases for replaced frames until
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

/// Builds the GPUI cutout through which Core Animation displays the page,
/// leaving later GPUI drawing above it. Until the page presents its first
/// frame there is no hole: a layer attached this frame may reach the screen a
/// frame after GPUI's, and the hole would show what lies beneath for that
/// frame. GPUI paints the page's background there meanwhile.
pub(in crate::platform) fn page_surface(
    presented: Option<&PresentedSurface>,
    _: PageSurfacePalette,
) -> AnyElement {
    if presented.is_none() {
        return gpui::Empty.into_any_element();
    }
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
