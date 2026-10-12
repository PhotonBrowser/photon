//! The page's right-click menu, built from the items the Engine offers and
//! shown as a native menu.

use gpui::{Context, MenuItem, Window};
use photon_shortcuts::ToggleZenMode;

use super::super::PageMenuItem;
use super::BrowserWindow;
use super::context_menu::show_window_context_menu;

/// Runs the item at `index` in the active page's context menu.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = photon, no_json)]
pub(super) struct ActivatePageMenuItem {
    index: usize,
}

impl BrowserWindow {
    /// Opens the active page's context menu, which the page asked for.
    pub(super) fn open_page_menu(&mut self, window: &Window, cx: &mut Context<Self>) {
        let Some(webview) = self.active_webview() else {
            return;
        };
        let Some(mut items) = webview
            .read(cx)
            .context_menu
            .as_ref()
            .map(|menu| page_menu_items(&menu.items))
        else {
            return;
        };
        // Zen mode hides every other way back to the chrome but its shortcut.
        if self.sidebar.zen() {
            items.extend([
                MenuItem::separator(),
                MenuItem::action("Exit Zen Mode", ToggleZenMode),
            ]);
        }
        show_window_context_menu(window, items, cx);
    }

    pub(super) fn activate_page_menu_item(
        &mut self,
        item: &ActivatePageMenuItem,
        cx: &mut Context<Self>,
    ) {
        if let Some(webview) = self.active_webview() {
            webview.update(cx, |view, _| view.activate_context_menu_item(item.index));
        }
    }
}

/// The Engine's items as native menu items, each running its own index.
fn page_menu_items(items: &[PageMenuItem]) -> Vec<MenuItem> {
    items
        .iter()
        .enumerate()
        .map(|(index, item)| match item {
            PageMenuItem::Separator => MenuItem::separator(),
            PageMenuItem::Action {
                label,
                enabled,
                checked,
            } => MenuItem::Action {
                name: label.clone(),
                action: Box::new(ActivatePageMenuItem { index }),
                os_action: None,
                checked: checked.unwrap_or(false),
                disabled: !enabled,
            },
        })
        .collect()
}
