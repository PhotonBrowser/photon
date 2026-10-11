//! Zooming the active page, and the chip that shows its zoom level.

use gpui::Context;
use std::time::Instant;

use super::super::super::engine::ZoomStep;
use super::BrowserWindow;

impl BrowserWindow {
    /// Zooms the active page and shows its new level in a chip.
    pub(super) fn zoom(&mut self, step: ZoomStep, cx: &mut Context<Self>) {
        if let Some(webview) = self.active_webview() {
            webview.update(cx, |view, _| view.zoom(step));
            self.zoom_shown_at = Some(Instant::now());
        }
        cx.notify();
    }
}
