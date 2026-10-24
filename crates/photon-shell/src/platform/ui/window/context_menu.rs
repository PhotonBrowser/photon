//! The browser's native context menu, opened by right-clicking empty
//! titlebar or sidebar space. Its items dispatch the browser's actions, so
//! they show their shortcuts and behave as the shortcuts do.

use crate::platform::shortcuts::{
    FindInPage, NewTab, NewWindow, OpenSettings, ResetZoom, ShowKeyboardShortcuts, ToggleDebugInfo,
    ToggleSidebarIcons, ToggleZenMode, ZoomIn, ZoomOut,
};
use gpui::{
    App, Context, Div, Menu, MenuItem, MouseButton, MouseDownEvent, Window, point, prelude::*, px,
};

use super::super::workspace_layout::WorkspaceLayout;
use super::BrowserWindow;

/// Opens a native menu anchored to this window's pointer position. Keeping an
/// A view anchor keeps the popup associated with this browser window instead
/// of placing it at a detached global pointer location.
pub(super) fn show_window_context_menu(window: &Window, items: Vec<MenuItem>, cx: &App) {
    let viewport = window.viewport_size();
    let pointer = window.mouse_position();
    let position = point(
        pointer.x.max(px(0.)).min(viewport.width),
        pointer.y.max(px(0.)).min(viewport.height),
    );
    drop(window.show_menu_at(items, position, px(0.), cx));
}

impl BrowserWindow {
    /// Opens the browser's context menu on a right-click in `area`'s empty
    /// space. Controls inside that have their own menu, or none, stop the
    /// click first.
    pub(super) fn with_browser_menu(&self, area: Div, cx: &mut Context<Self>) -> Div {
        area.on_mouse_down(
            MouseButton::Right,
            cx.listener(|this, _: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                this.show_browser_menu(window, cx);
            }),
        )
    }

    pub(super) fn show_browser_menu(&mut self, window: &Window, cx: &mut Context<Self>) {
        let items = self.browser_menu_items(cx);
        show_window_context_menu(window, items, cx);
    }

    /// New tabs and windows and settings; for a web page, finding, zoom and
    /// its debug details; then how the sidebar and chrome show.
    fn browser_menu_items(&self, cx: &Context<Self>) -> Vec<MenuItem> {
        let mut items = vec![
            MenuItem::action("New Tab", NewTab),
            MenuItem::action("New Window", NewWindow),
            MenuItem::separator(),
        ];
        if let Some(webview) = self.active_webview() {
            let debug_info = webview.read(cx).performance_overlay_enabled;
            items.extend([
                MenuItem::action("Find in Page…", FindInPage),
                MenuItem::submenu(Menu::new("Zoom").items([
                    MenuItem::action("Zoom In", ZoomIn),
                    MenuItem::action("Zoom Out", ZoomOut),
                    MenuItem::action("Actual Size", ResetZoom),
                ])),
                MenuItem::action("Debug Info", ToggleDebugInfo).checked(debug_info),
                MenuItem::separator(),
            ]);
        }
        if WorkspaceLayout::current(cx).has_sidebar() {
            let icons = self.sidebar.docked() && self.sidebar.collapsed();
            items.push(MenuItem::action("Sidebar as Icons", ToggleSidebarIcons).checked(icons));
        }
        items.extend([
            MenuItem::action("Zen Mode", ToggleZenMode).checked(self.sidebar.zen()),
            MenuItem::separator(),
            MenuItem::action("Keyboard Shortcuts", ShowKeyboardShortcuts),
            MenuItem::action("Settings…", OpenSettings),
        ]);
        items
    }
}
