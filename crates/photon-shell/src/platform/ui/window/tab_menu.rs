//! A tab's and a favourite's right-click menus, and the tab actions behind
//! them and drag-and-drop.

use gpui::{Context, Window, prelude::*};
use photon_core::{BrowserCommand, MAX_FAVOURITES, Shortcut, site_name};

use super::super::layout::v_stack;
use super::super::menu::{menu_action, menu_separator, menu_surface};
use super::super::motion::Transition;
use super::super::settings::Settings;
use super::super::theme::ThemeColors;
use super::BrowserWindow;
use super::content::TabContent;

impl BrowserWindow {
    /// Moves the tab at `from` to `to`, keeping the same tab active. Callers
    /// keep pinned tabs first; see `drop_tab`.
    pub(super) fn move_tab_to(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        let count = self.tabs.len();
        if from == to || from >= count || to >= count {
            return;
        }
        let active = self.tabs[self.active_tab].id;
        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);
        let subscription = self.tab_subscriptions.remove(from);
        self.tab_subscriptions.insert(to, subscription);
        let focus_handle = self.tab_focus_handles.remove(from);
        self.tab_focus_handles.insert(to, focus_handle);
        self.active_tab = self
            .tabs
            .iter()
            .position(|tab| tab.id == active)
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
        let Some(tab) = self.tabs.get(index) else {
            return;
        };
        match &tab.content {
            TabContent::Web(webview) => {
                let view = webview.read(cx);
                let address = view.has_page().then(|| view.state.url.clone());
                self.insert_tab(index + 1, address.as_deref(), window, cx);
            }
            TabContent::Page(page) => {
                let page = page.definition;
                self.insert_page(index + 1, page, window, cx);
            }
        }
    }

    pub(super) fn reload_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        match self.tabs.get(index).and_then(|tab| tab.content.webview()) {
            Some(tab) => tab.update(cx, |view, cx| {
                if let Err(error) = view.execute(BrowserCommand::Reload, cx) {
                    super::super::super::trace(format_args!("reloading tab: {error:#}"));
                }
            }),
            None => cx.notify(),
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
            .children(self.pin_action(index, palette, cx))
            .children(self.favourite_action(index, palette, cx))
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

    /// Pins the tab above today's tabs, or unpins it.
    fn pin_action(
        &self,
        index: usize,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        if !self.can_pin(index, cx) {
            return None;
        }
        let pinned = self.tabs[index].pin.is_some();
        Some(menu_action(
            "tab-menu-pin",
            if pinned { "Unpin Tab" } else { "Pin Tab" },
            6,
            Box::new(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.open_menu = None;
                this.toggle_pin(index, cx);
            })),
            palette,
        ))
    }

    /// Adds a web page's site to the favourites, or removes it.
    fn favourite_action(
        &self,
        index: usize,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let view = self.tabs.get(index)?.content.webview()?.read(cx);
        if !view.has_page() {
            return None;
        }
        let shortcut = Shortcut {
            title: view.state.title.clone(),
            url: view.state.url.clone(),
        };
        let sidebar = &Settings::get(cx).sidebar;
        let is_favourite = sidebar.favourite_for(&shortcut.url).is_some();
        if !is_favourite && sidebar.favourites.len() >= MAX_FAVOURITES {
            return None;
        }
        Some(menu_action(
            "tab-menu-favourite",
            if is_favourite {
                "Remove from Favourites"
            } else {
                "Add to Favourites"
            },
            5,
            Box::new(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.open_menu = None;
                if is_favourite {
                    this.remove_favourite(&shortcut.url, cx);
                } else {
                    this.add_favourite(index, shortcut.clone(), cx);
                }
            })),
            palette,
        ))
    }

    /// Adds the tab at `index` to the favourites as `shortcut`. The tab
    /// becomes the favourite's own, shown as its tile, so it leaves the tab
    /// list and the pinned tabs.
    fn add_favourite(&mut self, index: usize, shortcut: Shortcut, cx: &mut Context<Self>) {
        let url = shortcut.url.clone();
        let mut added = false;
        Settings::update(cx, |settings| {
            added = settings.sidebar.add_favourite(shortcut)
        });
        if !added {
            return;
        }
        // Marked first, as unpinning moves the tab.
        self.tabs[index].favourite = Some(url);
        if self.tabs[index].pin.is_some() {
            self.toggle_pin(index, cx);
        }
        cx.notify();
    }

    /// Removes the favourite for `url`'s site. Its tab returns to the tab
    /// list.
    fn remove_favourite(&mut self, url: &str, cx: &mut Context<Self>) {
        let site = site_name(url);
        for tab in &mut self.tabs {
            if tab
                .favourite
                .as_deref()
                .is_some_and(|owner| site_name(owner) == site)
            {
                tab.favourite = None;
            }
        }
        Settings::update(cx, |settings| settings.sidebar.remove_favourite(url));
        cx.notify();
    }

    /// The tab that belongs to the favourite at `url`, if it is open.
    pub(super) fn favourite_tab(&self, url: &str) -> Option<usize> {
        self.tabs
            .iter()
            .position(|tab| tab.favourite.as_deref() == Some(url))
    }

    /// The menu for the favourite at `index`: close its tab while open, or
    /// remove it.
    pub(super) fn favourite_menu(
        &self,
        index: usize,
        transition: Transition,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let url = Settings::get(cx)
            .sidebar
            .favourites
            .get(index)
            .map(|favourite| favourite.url.clone())
            .unwrap_or_default();
        let open_tab = self.favourite_tab(&url);
        let content = v_stack()
            .children(open_tab.map(|tab| {
                menu_action(
                    "favourite-menu-close",
                    "Close Tab",
                    0,
                    Box::new(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.open_menu = None;
                        this.close_tab(tab, window, cx);
                    })),
                    palette,
                )
            }))
            .child(menu_action(
                "favourite-menu-remove",
                "Remove from Favourites",
                1,
                Box::new(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.open_menu = None;
                    this.remove_favourite(&url, cx);
                })),
                palette,
            ));
        menu_surface(content, transition, palette)
    }
}
