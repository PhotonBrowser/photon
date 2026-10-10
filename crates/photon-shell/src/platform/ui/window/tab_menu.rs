//! A tab's right-click menu and the tab actions behind it and drag-and-drop.

use gpui::{Context, Window, prelude::*, px};
use photon_core::BrowserCommand;

use super::super::layout::v_stack;
use super::super::menu::{menu_action, menu_separator, menu_surface};
use super::super::motion::Transition;
use super::super::{metrics, theme::ThemeColors};
use super::BrowserWindow;

impl BrowserWindow {
    /// Moves the tab at `from` to `to`, keeping the same tab active.
    pub(super) fn move_tab(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        let count = self.tabs.len();
        if from == to || from >= count || to >= count {
            return;
        }
        let active = self.active_webview().entity_id();
        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);
        let subscription = self.tab_subscriptions.remove(from);
        self.tab_subscriptions.insert(to, subscription);
        let focus_handle = self.tab_focus_handles.remove(from);
        self.tab_focus_handles.insert(to, focus_handle);
        self.active_tab = self
            .tabs
            .iter()
            .position(|tab| tab.entity_id() == active)
            .unwrap_or(0);
        cx.notify();
    }

    /// Opens the tab's page again in a new tab beside it.
    pub(super) fn duplicate_tab(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(view) = self.tabs.get(index).map(|tab| tab.read(cx)) else {
            return;
        };
        let address = view.has_page().then(|| view.state.url.clone());
        self.insert_tab(index + 1, address.as_deref(), window, cx);
    }

    pub(super) fn reload_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get(index) {
            tab.update(cx, |view, cx| {
                if let Err(error) = view.execute(BrowserCommand::Reload, cx) {
                    super::super::super::trace(format_args!("reloading tab: {error:#}"));
                }
            });
        }
    }

    /// Closes every tab but the one at `index`, which becomes active.
    pub(super) fn close_other_tabs(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.tabs.len() {
            return;
        }
        self.activate_tab(index, true, window, cx);
        for other in (0..self.tabs.len()).rev().filter(|other| *other != index) {
            self.close_tab(other, window, cx);
        }
    }

    /// Closes the tabs after the one at `index`.
    pub(super) fn close_tabs_to_right(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.tabs.len() {
            return;
        }
        if self.active_tab > index {
            self.activate_tab(index, true, window, cx);
        }
        for right in (index + 1..self.tabs.len()).rev() {
            self.close_tab(right, window, cx);
        }
    }

    /// The menu for the tab at `index`.
    pub(super) fn tab_menu(
        &self,
        index: usize,
        transition: Transition,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let count = self.tabs.len();
        let mut content = v_stack()
            .gap(px(metrics::MENU_ITEM_GAP))
            .child(menu_action(
                "tab-menu-reload",
                "Reload Tab",
                0,
                Box::new(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.open_menu = None;
                    this.reload_tab(index, cx);
                })),
                palette,
            ))
            .child(menu_action(
                "tab-menu-duplicate",
                "Duplicate Tab",
                1,
                Box::new(cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.open_menu = None;
                    this.duplicate_tab(index, window, cx);
                })),
                palette,
            ))
            .child(menu_separator(palette))
            .child(menu_action(
                "tab-menu-close",
                "Close Tab",
                2,
                Box::new(cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.open_menu = None;
                    this.close_tab(index, window, cx);
                })),
                palette,
            ));
        if count > 1 {
            content = content.child(menu_action(
                "tab-menu-close-others",
                "Close Other Tabs",
                3,
                Box::new(cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.open_menu = None;
                    this.close_other_tabs(index, window, cx);
                })),
                palette,
            ));
        }
        if index + 1 < count {
            content = content.child(menu_action(
                "tab-menu-close-right",
                "Close Tabs to the Right",
                4,
                Box::new(cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.open_menu = None;
                    this.close_tabs_to_right(index, window, cx);
                })),
                palette,
            ));
        }
        menu_surface(content, transition, palette)
    }
}
