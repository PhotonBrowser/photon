//! The page's context menu: the items the Engine offers and running one.

use gpui::{Context, Pixels, Point, SharedString, point, px};

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
    /// Where the menu opens, in window coordinates.
    pub(in crate::platform) position: Point<Pixels>,
    pub(in crate::platform) items: Vec<PageMenuItem>,
}

impl PhotonWebView {
    /// Shows the page's context menu at `(x, y)` in the page.
    pub(in crate::platform) fn show_context_menu(
        &mut self,
        x: f64,
        y: f64,
        items: Vec<PageMenuItem>,
        cx: &mut Context<Self>,
    ) {
        let position = self.input.viewport_origin() + point(px(x as f32), px(y as f32));
        self.context_menu = Some(PageMenu { position, items });
        cx.emit(WebViewEvent::ContextMenuRequested);
    }

    /// Runs the context menu item at `index`. The items stay until the next
    /// request, so the closing menu can show them as it animates away.
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
}
