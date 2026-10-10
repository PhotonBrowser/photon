//! Favourite sites: their tiles in the sidebar, each with a tab of its own
//! as in Arc, adding and removing them from a tab's menu, and a tile's menu.

use gpui::{Context, ImageSource, MouseDownEvent, ObjectFit, Window, img, prelude::*, px};
use photon_core::{MAX_FAVOURITES, Shortcut, site_name};

use super::super::history::BrowsingHistory;
use super::super::icons::globe_icon;
use super::super::layout::v_stack;
use super::super::menu::{menu_action, menu_surface};
use super::super::motion::Transition;
use super::super::settings::Settings;
use super::super::sidebar::FavouriteTile;
use super::super::{metrics, theme::ThemeColors};
use super::BrowserWindow;
use super::menu::OpenMenu;

impl BrowserWindow {
    pub(super) fn favourite_tiles(
        &self,
        favourites: &[Shortcut],
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> Vec<FavouriteTile> {
        let active_favourite = self.tabs[self.active_tab].favourite.clone();
        favourites
            .iter()
            .enumerate()
            .map(|(index, favourite)| {
                let icon = match BrowsingHistory::favicon(&favourite.url, cx) {
                    Some(favicon) => img(ImageSource::Render(favicon.image))
                        .size(px(metrics::SIDEBAR_FAVOURITE_ICON_SIZE))
                        .object_fit(ObjectFit::Contain)
                        .into_any_element(),
                    None => {
                        globe_icon(palette.text_secondary, metrics::SIDEBAR_FAVOURITE_ICON_SIZE)
                            .into_any_element()
                    }
                };
                let url = favourite.url.clone();
                FavouriteTile {
                    label: if favourite.title.trim().is_empty() {
                        site_name(&favourite.url)
                    } else {
                        favourite.title.clone()
                    },
                    icon,
                    active: active_favourite.as_deref() == Some(favourite.url.as_str()),
                    on_open: Box::new(cx.listener(move |this, _, window, cx| {
                        this.open_favourite(&url, window, cx);
                    })),
                    on_context_menu: Box::new(cx.listener(
                        move |this, event: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            this.open_menu = Some(OpenMenu::Favourite(index, event.position));
                            cx.notify();
                        },
                    )),
                }
            })
            .collect()
    }

    /// Switches to the favourite's tab, or opens the favourite in a tab of
    /// its own.
    fn open_favourite(&mut self, url: &str, window: &mut Window, cx: &mut Context<Self>) {
        match self.favourite_tab(url) {
            Some(index) => self.activate_tab(index, true, window, cx),
            None => {
                let index = self.tabs.len();
                self.insert_tab(index, Some(url), window, cx);
                // The new tab is the tile's, so it never shows in the list.
                self.tabs[self.active_tab].favourite = Some(url.to_owned());
                cx.notify();
            }
        }
    }

    /// Adds a web page's site to the favourites, or removes it.
    pub(super) fn favourite_action(
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
