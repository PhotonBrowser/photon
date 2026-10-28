//! Element fullscreen: the page asks, the window fills itself with the page,
//! and Escape asks the page to leave, as Ladybird's Qt browser does.

use gpui::{Context, KeyDownEvent};

use super::{PhotonWebView, WebViewEvent};

impl PhotonWebView {
    pub(in crate::platform) fn page_fullscreen_requested(
        &mut self,
        fullscreen: bool,
        cx: &mut Context<Self>,
    ) {
        if self.page_fullscreen == fullscreen {
            return;
        }
        self.page_fullscreen = fullscreen;
        cx.emit(WebViewEvent::FullscreenChanged(fullscreen));
        cx.notify();
    }

    /// Reports that the window now shows the page fullscreen, or no longer.
    pub(in crate::platform) fn set_shown_fullscreen(&mut self, fullscreen: bool) {
        self.session.set_fullscreen(fullscreen);
    }

    /// Asks the page to leave fullscreen, as when it is hidden or the window
    /// leaves fullscreen on its own.
    pub(in crate::platform) fn exit_page_fullscreen(&mut self) {
        if self.page_fullscreen {
            self.session.exit_fullscreen();
        }
    }

    /// Escape leaves fullscreen rather than reaching the page.
    pub(super) fn fullscreen_key_down(&mut self, event: &KeyDownEvent) -> bool {
        if self.page_fullscreen && event.keystroke.key == "escape" {
            self.session.exit_fullscreen();
            return true;
        }
        false
    }
}
