//! The browser menu, opened from the toolbar or by right-clicking the titlebar.

use gpui::{
    Anchor, ClickEvent, Context, MouseButton, MouseDownEvent, Point, Window, WindowAppearance,
    anchored, div, point, prelude::*, px,
};
use photon_core::BrowserCommand;

use super::super::super::engine::PopupPolicy;
use super::super::layout::v_stack;
use super::super::menu::{
    menu_action, menu_checkbox, menu_radio, menu_section, menu_separator, menu_surface,
};
use super::super::{metrics, theme::ThemeColors};
use super::BrowserWindow;

/// Where the open browser menu is anchored.
#[derive(Clone, Copy)]
pub(super) enum OpenMenu {
    Context(Point<gpui::Pixels>),
    Toolbar(Point<gpui::Pixels>),
}

/// Anchors the toolbar menu below the menu button's bottom-right corner.
pub(super) fn toolbar_menu_anchor(event: &ClickEvent) -> Point<gpui::Pixels> {
    match event {
        ClickEvent::Keyboard(event) => event.bounds.bottom_right(),
        ClickEvent::Mouse(_) | ClickEvent::Touch(_) => {
            let position = event.position();
            let button_center_offset = px(metrics::TOOLBAR_BUTTON_SIZE / 2.0);
            point(
                position.x + button_center_offset,
                position.y + button_center_offset,
            )
        }
    }
}

impl BrowserWindow {
    /// The open menu over a click-away backdrop that closes it.
    pub(super) fn open_menu_overlay(
        &self,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let (anchor, position) = match self.open_menu? {
            OpenMenu::Context(position) => (Anchor::TopLeft, position),
            OpenMenu::Toolbar(position) => (Anchor::TopRight, position),
        };
        Some(
            div()
                .absolute()
                .inset_0()
                .on_mouse_down(MouseButton::Left, cx.listener(Self::close_menu))
                .on_mouse_down(MouseButton::Right, cx.listener(Self::close_menu))
                .child(
                    anchored()
                        .anchor(anchor)
                        .position(position)
                        .snap_to_window()
                        .child(self.browser_menu(palette, cx)),
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
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let performance_overlay_enabled =
            self.active_webview().read(cx).performance_overlay_enabled;
        let theme = self.theme.get();
        let popup_policy = self.runtime.popup_policy();
        let theme_radio = |id, label, tab_index, appearance: Option<WindowAppearance>| {
            menu_radio(
                id,
                label,
                tab_index,
                theme == appearance,
                Box::new(cx.listener(move |this: &mut Self, _, _, cx| {
                    cx.stop_propagation();
                    this.open_menu = None;
                    this.set_theme(appearance, cx);
                })),
                palette,
            )
        };
        let popup_radio = |id, label, tab_index, policy: PopupPolicy| {
            menu_radio(
                id,
                label,
                tab_index,
                popup_policy == policy,
                Box::new(cx.listener(move |this: &mut Self, _, _, cx| {
                    cx.stop_propagation();
                    this.open_menu = None;
                    this.runtime.set_popup_policy(policy);
                    cx.notify();
                })),
                palette,
            )
        };

        let content = v_stack()
            .gap(px(metrics::MENU_ITEM_GAP))
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
                "menu-find",
                "Find in Page…",
                2,
                Box::new(cx.listener(|this, _, window, cx| {
                    cx.stop_propagation();
                    this.open_menu = None;
                    this.open_find_bar(window, cx);
                })),
                palette,
            ))
            .child(menu_checkbox(
                "menu-debug-info",
                "Debug info",
                3,
                performance_overlay_enabled,
                Box::new(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.toggle_performance_overlay(cx);
                })),
                palette,
            ))
            .child(menu_separator(palette))
            .child(menu_section("Theme", palette))
            .child(theme_radio("menu-theme-system", "System", 4, None))
            .child(theme_radio(
                "menu-theme-light",
                "Light",
                5,
                Some(WindowAppearance::Light),
            ))
            .child(theme_radio(
                "menu-theme-dark",
                "Dark",
                6,
                Some(WindowAppearance::Dark),
            ))
            .child(menu_separator(palette))
            .child(menu_section("Pop-up windows", palette))
            .child(popup_radio("menu-popups-ask", "Ask", 6, PopupPolicy::Ask))
            .child(popup_radio(
                "menu-popups-allow",
                "Allow",
                7,
                PopupPolicy::Allow,
            ))
            .child(popup_radio(
                "menu-popups-block",
                "Block",
                8,
                PopupPolicy::Block,
            ));

        menu_surface(content, palette)
    }

    /// Applies a light or dark theme, or follows the system for `None`.
    fn set_theme(&mut self, appearance: Option<WindowAppearance>, cx: &mut Context<Self>) {
        self.theme.set(appearance);
        cx.set_window_appearance(appearance);

        for tab in self.tabs.clone() {
            tab.update(cx, |_, cx| cx.notify());
        }
        self.omnibox.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    fn toggle_performance_overlay(&mut self, cx: &mut Context<Self>) {
        self.open_menu = None;
        self.active_webview().update(cx, |view, cx| {
            view.set_performance_overlay_enabled(!view.performance_overlay_enabled);
            cx.notify();
        });
        cx.notify();
    }
}
