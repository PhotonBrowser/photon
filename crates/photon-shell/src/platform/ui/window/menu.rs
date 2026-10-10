//! The browser menu, opened from the toolbar, the sidebar's footer, or by
//! right-clicking the titlebar, and the overlay that shows it or another menu.

use gpui::{
    Anchor, Context, MouseButton, MouseDownEvent, Point, SharedString, Window, anchored, div,
    prelude::*,
};
use photon_core::BrowserCommand;

use super::super::super::engine::ZoomStep;
use super::super::layout::v_stack;
use super::super::menu::{MENU_MOTION, menu_action, menu_checkbox, menu_stepper, menu_surface};
use super::super::motion::Transition;
use super::super::pages::SETTINGS;
use super::super::theme::ThemeColors;
use super::BrowserWindow;

/// Where the open browser menu is anchored.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum OpenMenu {
    /// Right-clicking the sidebar's top row.
    Context(Point<gpui::Pixels>),
    /// The toolbar's menu button; the menu opens below it.
    Toolbar(Point<gpui::Pixels>),
    /// The sidebar footer's menu button; the menu opens above it.
    Sidebar(Point<gpui::Pixels>),
    /// The menu for the favourite at this index.
    Favourite(usize, Point<gpui::Pixels>),
    /// The menu for the tab at this index.
    Tab(usize, Point<gpui::Pixels>),
    /// The active page's context menu.
    Page(Point<gpui::Pixels>),
}

impl BrowserWindow {
    /// The open menu over a click-away backdrop that closes it, or the menu
    /// that just closed as it animates away.
    pub(super) fn open_menu_overlay(
        &mut self,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let (open_menu, transition) = self.menu_presence.sync(self.open_menu, MENU_MOTION, cx)?;
        let (anchor, position) = match open_menu {
            OpenMenu::Context(position)
            | OpenMenu::Tab(_, position)
            | OpenMenu::Favourite(_, position)
            | OpenMenu::Page(position) => (Anchor::TopLeft, position),
            OpenMenu::Toolbar(position) => (Anchor::TopRight, position),
            OpenMenu::Sidebar(position) => (Anchor::BottomRight, position),
        };
        let menu = match open_menu {
            OpenMenu::Tab(index, _) => self
                .tab_menu(index, transition, palette, cx)
                .into_any_element(),
            OpenMenu::Page(_) => self.page_menu(transition, palette, cx).into_any_element(),
            OpenMenu::Favourite(index, _) => self
                .favourite_menu(index, transition, palette, cx)
                .into_any_element(),
            OpenMenu::Context(_) | OpenMenu::Toolbar(_) | OpenMenu::Sidebar(_) => self
                .browser_menu(transition, palette, cx)
                .into_any_element(),
        };
        let mut overlay = div().absolute().inset_0();
        // A closing menu lets clicks through to what is beneath.
        if transition == Transition::Enter {
            overlay = overlay
                .on_mouse_down(MouseButton::Left, cx.listener(Self::close_menu))
                .on_mouse_down(MouseButton::Right, cx.listener(Self::close_menu));
        }
        Some(
            overlay.child(
                anchored()
                    .anchor(anchor)
                    .position(position)
                    .snap_to_window()
                    .child(menu),
            ),
        )
    }

    fn close_menu(&mut self, _: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.open_menu = None;
        cx.stop_propagation();
        cx.notify();
    }

    fn browser_menu(
        &self,
        transition: Transition,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let active_webview = self.active_webview();
        let has_page = active_webview.is_some();
        let performance_overlay_enabled = active_webview
            .as_ref()
            .is_some_and(|webview| webview.read(cx).performance_overlay_enabled);
        let zoom_percent = active_webview
            .as_ref()
            .map_or(100, |webview| webview.read(cx).zoom_percent());
        let mut content = v_stack()
            .child(menu_action(
                "menu-new-tab",
                "New Tab",
                0,
                Box::new(cx.listener(|this, _, window, cx| {
                    cx.stop_propagation();
                    this.dispatch_command(BrowserCommand::NewTab, window, cx);
                })),
                palette,
            ))
            .child(menu_action(
                "menu-new-window",
                "New Window",
                1,
                Box::new(cx.listener(|this, _, window, cx| {
                    cx.stop_propagation();
                    this.dispatch_command(BrowserCommand::NewWindow, window, cx);
                })),
                palette,
            ))
            .child(menu_action(
                "menu-settings",
                "Settings",
                2,
                Box::new(cx.listener(|this, _, window, cx| {
                    cx.stop_propagation();
                    this.open_page_tab(SETTINGS, window, cx);
                })),
                palette,
            ));

        if has_page {
            content = content
                .child(menu_action(
                    "menu-find",
                    "Find in Page…",
                    3,
                    Box::new(cx.listener(|this, _, window, cx| {
                        cx.stop_propagation();
                        this.open_menu = None;
                        this.open_find_bar(window, cx);
                    })),
                    palette,
                ))
                .child(menu_stepper(
                    "Zoom",
                    SharedString::from(format!("{zoom_percent}%")),
                    Box::new(cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.zoom(ZoomStep::Out, cx);
                    })),
                    Box::new(cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.zoom(ZoomStep::Reset, cx);
                    })),
                    Box::new(cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.zoom(ZoomStep::In, cx);
                    })),
                    palette,
                ))
                .child(menu_checkbox(
                    "menu-debug-info",
                    "Debug info",
                    4,
                    performance_overlay_enabled,
                    Box::new(cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.toggle_performance_overlay(cx);
                    })),
                    palette,
                ));
        }

        menu_surface(content, transition, palette)
    }

    fn toggle_performance_overlay(&mut self, cx: &mut Context<Self>) {
        self.open_menu = None;
        if let Some(webview) = self.active_webview() {
            webview.update(cx, |view, cx| {
                view.set_performance_overlay_enabled(!view.performance_overlay_enabled);
                cx.notify();
            });
        }
        cx.notify();
    }
}
