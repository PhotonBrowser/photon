//! The page's context menu: the items the Engine offers and running one.

use gpui::{Context, SharedString};

use super::{PhotonWebView, WebViewEvent};

/// An item in a page's context menu.
pub(in crate::platform) enum PageMenuItem {
    Action {
        label: SharedString,
        enabled: bool,
        /// Whether a checkable action is checked; `None` when it is not checkable.
        checked: Option<bool>,
    },
    Separator,
}

/// A context menu the page asked for.
pub(in crate::platform) struct PageMenu {
    pub(in crate::platform) items: Vec<PageMenuItem>,
}

impl PhotonWebView {
    /// Asks the window to show the page's context menu.
    pub(in crate::platform) fn show_context_menu(
        &mut self,
        items: Vec<PageMenuItem>,
        cx: &mut Context<Self>,
    ) {
        self.context_menu = Some(PageMenu { items });
        cx.emit(WebViewEvent::ContextMenuRequested);
    }

    /// Runs the context menu item at `index`. The items stay until the next
    /// request, since the menu reports its choice after it closes.
    pub(in super::super) fn activate_context_menu_item(&mut self, index: usize) {
        self.session.activate_context_menu_item(index);
    }

    /// Asks the window to open `url` in a new tab.
    pub(in crate::platform) fn open_in_new_tab(
        &mut self,
        url: String,
        activate: bool,
        cx: &mut Context<Self>,
    ) {
        cx.emit(WebViewEvent::OpenInNewTab { url, activate });
    }

    /// Asks the window to open `url` in a new window, private or not.
    pub(in crate::platform) fn open_in_new_window(
        &mut self,
        url: String,
        private: bool,
        cx: &mut Context<Self>,
    ) {
        cx.emit(WebViewEvent::OpenInNewWindow { url, private });
    }

    /// Asks where to save a link or image, in the system's save panel,
    /// starting in the Downloads folder.
    pub(in crate::platform) fn request_download_path(
        &mut self,
        request_id: u64,
        suggested_name: String,
        cx: &mut Context<Self>,
    ) {
        let directory = std::env::home_dir()
            .map(|home| home.join("Downloads"))
            .filter(|downloads| downloads.is_dir())
            .unwrap_or_else(std::env::temp_dir);
        let chosen = cx.prompt_for_new_path(
            &directory,
            (!suggested_name.is_empty()).then_some(suggested_name.as_str()),
        );
        cx.spawn(async move |this, cx| {
            let path = chosen.await.ok().and_then(Result::ok).flatten();
            this.update(cx, |view, _| {
                view.session
                    .download_path_chosen(request_id, path.as_deref());
            })
            .ok();
        })
        .detach();
    }

    /// A shift-clicked link, for the window to show in a preview.
    pub(in crate::platform) fn peek(&mut self, url: String, cx: &mut Context<Self>) {
        cx.emit(WebViewEvent::Peek(url));
    }
}
