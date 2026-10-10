//! The page's zoom level.

use gpui::Context;

use super::super::super::engine::ZoomStep;
use super::PhotonWebView;

impl PhotonWebView {
    pub(in super::super) fn zoom(&mut self, step: ZoomStep) {
        self.session.zoom(step);
    }

    pub(in crate::platform) fn set_zoom_level(&mut self, zoom_level: f64, cx: &mut Context<Self>) {
        super::super::super::trace(format_args!("zoom level {zoom_level}"));
        if self.zoom_level != zoom_level {
            self.zoom_level = zoom_level;
            self.state_changed(cx);
        }
    }

    /// The zoom level as a whole percentage, such as 110.
    pub(in super::super) fn zoom_percent(&self) -> u32 {
        (self.zoom_level * 100.0).round() as u32
    }
}
