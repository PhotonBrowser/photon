//! Notices and dialogs over the page: crash alerts and JavaScript dialogs.

use gpui::{Context, Entity, Window, prelude::*};
use photon_core::BrowserCommand;

use super::super::crash_alert::crash_alert;
use super::super::js_dialog::JavaScriptDialog;
use super::super::theme::ThemeColors;
use super::super::{ClickHandler, PhotonWebView};
use super::BrowserWindow;

impl BrowserWindow {
    /// Shows the active tab's JavaScript dialog, if it has one, and returns
    /// focus to the page when it closes.
    pub(super) fn sync_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let webview = self.active_webview();
        let open = webview.read(cx).dialogs.open().cloned();
        let shown = self
            .dialog
            .as_ref()
            .map(|dialog| dialog.read(cx).request().clone());
        if open == shown {
            return;
        }
        let had_focus = self
            .dialog
            .take()
            .is_some_and(|dialog| dialog.read(cx).contains_focus(window, cx));
        match open {
            Some(request) => {
                let dialog = cx.new(|cx| JavaScriptDialog::new(webview, request, cx));
                dialog.update(cx, |dialog, cx| dialog.focus(window, cx));
                self.dialog = Some(dialog);
            }
            None if had_focus => {
                let page_focus = webview.read(cx).focus_handle.clone();
                window.focus(&page_focus, cx);
            }
            None => {}
        }
        cx.notify();
    }

    fn crashed_tab(&self, cx: &Context<Self>) -> Option<Entity<PhotonWebView>> {
        self.tabs
            .iter()
            .find(|tab| tab.read(cx).crash_alert)
            .cloned()
    }

    /// Dismisses a shown crash alert, returning whether there was one.
    pub(super) fn dismiss_crash_alert(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(tab) = self.crashed_tab(cx) else {
            return false;
        };
        tab.update(cx, |view, cx| {
            view.crash_alert = false;
            cx.notify();
        });
        true
    }

    /// The alert for a tab whose page process crashed, offering to reload it.
    pub(super) fn crash_alert(
        &self,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let tab = self.crashed_tab(cx)?;
        let on_reload: ClickHandler = Box::new(cx.listener(move |_, _, _, cx| {
            tab.update(cx, |view, cx| {
                view.crash_alert = false;
                if let Err(error) = view.execute(BrowserCommand::Reload, cx) {
                    eprintln!("Photon Engine: retrying crashed page failed: {error:#}");
                }
                cx.notify();
            });
            cx.notify();
        }));
        let on_dismiss: ClickHandler = Box::new(cx.listener(|this, _, _, cx| {
            this.dismiss_crash_alert(cx);
            cx.notify();
        }));
        Some(crash_alert(palette, on_reload, on_dismiss))
    }
}
