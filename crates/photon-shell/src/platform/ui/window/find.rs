//! Opening, stepping through and closing the find bar.

use gpui::{Context, Window, prelude::*};

use super::super::find_bar::{CloseFindBar, FindBar};
use super::BrowserWindow;

impl BrowserWindow {
    /// Opens the find bar for the active tab, or focuses it when it is open.
    pub(super) fn open_find_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let bar = match &self.find_bar {
            Some((bar, _)) => bar.clone(),
            None => {
                let Some(webview) = self.active_webview() else {
                    return;
                };
                let bar = cx.new(|cx| FindBar::new(webview, cx));
                let subscription =
                    cx.subscribe_in(&bar, window, |this, _, _: &CloseFindBar, window, cx| {
                        this.close_find_bar(true, window, cx);
                    });
                self.find_bar = Some((bar.clone(), subscription));
                bar
            }
        };
        bar.update(cx, |bar, cx| bar.focus(window, cx));
        cx.notify();
    }

    /// Goes to the next or previous match, opening the bar if it is closed.
    pub(super) fn find_step(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        match &self.find_bar {
            Some((bar, _)) => bar.update(cx, |bar, cx| bar.step(forward, cx)),
            None => self.open_find_bar(window, cx),
        }
    }

    /// Closes the find bar and clears its highlights, returning focus to the
    /// page when `focus_page` is set.
    pub(super) fn close_find_bar(
        &mut self,
        focus_page: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.find_bar.take().is_none() {
            return;
        }
        if let Some(webview) = self.active_webview() {
            webview.update(cx, |view, cx| {
                view.end_find(cx);
                if focus_page {
                    window.focus(&view.focus_handle, cx);
                }
            });
        }
        cx.notify();
    }
}
