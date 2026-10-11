//! The window's title, which follows the page it shows, so Mission
//! Control, ⌘` and the Window menu name the page.

use gpui::{App, Window};

use super::BrowserWindow;
use super::command_bar::tab_summary;

impl BrowserWindow {
    /// Titles the window after the active tab, or the browser with none.
    pub(super) fn follow_page_title(&mut self, window: &mut Window, cx: &App) {
        let title = self
            .active_tab
            .and_then(|index| self.tabs.get(index))
            .map(|tab| tab_summary(tab, cx).title)
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| photon_brand::NAME.to_owned());
        if self.window_title != title {
            window.set_window_title(&title);
            self.window_title = title;
        }
    }
}
