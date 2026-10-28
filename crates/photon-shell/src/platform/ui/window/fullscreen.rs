//! A page's element fullscreen: the window goes fullscreen, if it is not
//! already, and shows only that page until the page leaves fullscreen.

use gpui::{Context, Entity, Window};

use super::BrowserWindow;
use crate::platform::ui::webview::PhotonWebView;

pub(super) struct PageFullscreen {
    webview: Entity<PhotonWebView>,
    /// The window was fullscreen already, so leaving keeps it that way.
    window_was_fullscreen: bool,
    /// The window has finished entering fullscreen.
    entered: bool,
    /// The page was asked to leave and has not answered yet.
    exiting: bool,
}

impl BrowserWindow {
    pub(super) fn page_fullscreen_changed(
        &mut self,
        webview: &Entity<PhotonWebView>,
        fullscreen: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if fullscreen {
            if self.active_webview().as_ref() != Some(webview) || self.page_fullscreen.is_some() {
                webview.update(cx, |view, _| view.exit_page_fullscreen());
                return;
            }
            let window_was_fullscreen = window.is_fullscreen();
            if !window_was_fullscreen {
                window.toggle_fullscreen();
            }
            self.page_fullscreen = Some(PageFullscreen {
                webview: webview.clone(),
                window_was_fullscreen,
                entered: window_was_fullscreen,
                exiting: false,
            });
            webview.update(cx, |view, _| view.set_shown_fullscreen(true));
        } else {
            let Some(state) = self
                .page_fullscreen
                .take_if(|state| &state.webview == webview)
            else {
                return;
            };
            if !state.window_was_fullscreen && window.is_fullscreen() {
                window.toggle_fullscreen();
            }
            webview.update(cx, |view, _| view.set_shown_fullscreen(false));
        }
        cx.notify();
    }

    /// Whether the window shows only a fullscreen page. Leaves fullscreen
    /// when another tab became active or the user took the window out of
    /// fullscreen.
    pub(super) fn sync_page_fullscreen(&mut self, window: &Window, cx: &mut Context<Self>) -> bool {
        let active = self.active_webview();
        let Some(state) = self.page_fullscreen.as_mut() else {
            return false;
        };
        let window_fullscreen = window.is_fullscreen();
        state.entered |= window_fullscreen;
        let left_window = state.entered && !window_fullscreen;
        if !state.exiting && (active.as_ref() != Some(&state.webview) || left_window) {
            state.exiting = true;
            if left_window {
                // Nothing to restore: the window is out of fullscreen already.
                state.window_was_fullscreen = true;
            }
            let webview = state.webview.clone();
            webview.update(cx, |view, _| view.exit_page_fullscreen());
        }
        true
    }
}
