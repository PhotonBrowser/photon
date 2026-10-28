//! Favourite sites: their tiles in the sidebar, each with a tab of its own
//! as in Arc, adding and removing them from a tab's menu, and a tile's menu.

use gpui::{
    Context, ImageSource, MenuItem, MouseDownEvent, ObjectFit, Window, img, prelude::*, px,
};
use photon_core::{MAX_FAVOURITES, Shortcut, SpaceId, site_name};

use super::super::Favicon;

use super::super::history::BrowsingHistory;
use super::super::icons::globe_icon;
use super::super::settings::Settings;
use super::super::sidebar::FavouriteTile;
use super::super::{metrics, theme::ThemeColors};
use super::BrowserWindow;
use super::content::{BrowserTab, TabContent};
use super::menu::{MenuCommand, command_item};
use super::pinned::pin_for;

impl BrowserWindow {
    /// The space's favourites as tiles, with smaller icons when the sidebar
    /// is collapsed.
    pub(super) fn favourite_tiles(
        &self,
        favourites: &[Shortcut],
        collapsed: bool,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> Vec<FavouriteTile> {
        let active_space = self.current_space(cx);
        let active_favourite = self
            .active_tab
            .and_then(|index| self.tabs.get(index))
            .filter(|tab| tab.space == active_space)
            .and_then(|tab| tab.favourite.clone());
        let icon_size = if collapsed {
            metrics::SIDEBAR_COLLAPSED_ICON_SIZE
        } else {
            metrics::SIDEBAR_FAVOURITE_ICON_SIZE
        };
        favourites
            .iter()
            .enumerate()
            .map(|(index, favourite)| {
                // The open tab's icon is current; the saved one stands in
                // while the favourite has no tab.
                let favicon = self
                    .favourite_tab(&favourite.url, active_space)
                    .and_then(|tab| tab_favicon(&self.tabs[tab], cx))
                    .or_else(|| BrowsingHistory::favicon(&favourite.url, cx));
                let icon = match favicon {
                    Some(favicon) => img(ImageSource::Render(favicon.image))
                        .size(px(icon_size))
                        .object_fit(ObjectFit::Contain)
                        .into_any_element(),
                    None => globe_icon(palette.text_secondary, icon_size).into_any_element(),
                };
                let url = favourite.url.clone();
                FavouriteTile {
                    url: favourite.url.clone(),
                    appearing: self.added_favourite.as_deref() == Some(favourite.url.as_str()),
                    label: if favourite.title.trim().is_empty() {
                        site_name(&favourite.url)
                    } else {
                        favourite.title.clone()
                    },
                    icon,
                    active: active_favourite.as_deref() == Some(favourite.url.as_str()),
                    on_open: Box::new(cx.listener(move |this, _, window, cx| {
                        this.open_favourite(&url, active_space, window, cx);
                    })),
                    on_context_menu: Box::new(cx.listener(
                        move |this, _: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            let items = this.favourite_menu_items(index, cx);
                            this.show_context_menu(window, items, cx);
                        },
                    )),
                }
            })
            .collect()
    }

    /// Switches to the favourite's tab, or opens the favourite in a tab of
    /// its own.
    fn open_favourite(
        &mut self,
        url: &str,
        space_id: SpaceId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.favourite_tab(url, space_id) {
            Some(index) => self.activate_tab(index, true, window, cx),
            None => {
                let index = self.tabs.len();
                self.insert_tab(index, Some(url), window, cx);
                // The new tab is the tile's, so it never shows in the list.
                if let Some(active_tab) = self.active_tab {
                    self.tabs[active_tab].favourite = Some(url.to_owned());
                }
                cx.notify();
            }
        }
    }

    /// The tab menu's item adding a web page's site to the favourites, or
    /// removing it.
    pub(super) fn favourite_item(&self, index: usize, cx: &Context<Self>) -> Option<MenuItem> {
        let tab = self.tabs.get(index)?;
        if matches!(&tab.content, super::content::TabContent::Page(_)) {
            return None;
        }
        let space = tab.space;
        if let Some(url) = tab.favourite.clone() {
            return Some(command_item(
                "Remove from Favourites",
                MenuCommand::RemoveFavourite { url, space },
            ));
        }
        let shortcut = pin_for(tab, cx)?;
        let saved = Settings::get(cx).spaces.get(space)?;
        if saved.favourite_for(&shortcut.url).is_some() {
            return None;
        }
        (saved.favourites.len() < MAX_FAVOURITES).then(|| {
            command_item(
                "Add to Favourites",
                MenuCommand::AddFavourite {
                    tab: index,
                    shortcut,
                },
            )
        })
    }

    /// Adds the tab at `index` to the favourites as `shortcut`. The tab
    /// becomes the favourite's own, shown as its tile, so it leaves the tab
    /// list and the pinned tabs.
    pub(super) fn add_favourite(
        &mut self,
        index: usize,
        shortcut: Shortcut,
        cx: &mut Context<Self>,
    ) {
        let url = shortcut.url.clone();
        let Some(space_id) = self.tabs.get(index).map(|tab| tab.space) else {
            return;
        };
        let full = Settings::get(cx)
            .spaces
            .get(space_id)
            .is_some_and(|space| space.favourites.len() >= MAX_FAVOURITES);
        if full {
            self.show_action_alert("Favourites are full", cx);
            return;
        }
        let mut added = false;
        Settings::update(cx, |settings| {
            if let Some(space) = settings.spaces.get_mut(space_id) {
                added = space.add_favourite(shortcut);
            }
        });
        if !added {
            return;
        }
        // Kept under the favourite's own address, so the tile keeps its icon
        // after the tab closes or moves on to another page.
        if let Some(favicon) = tab_favicon(&self.tabs[index], cx) {
            BrowsingHistory::set_favicon(&url, &favicon, cx);
        }
        self.added_favourite = Some(url.clone());
        // Marked first, as unpinning moves the tab.
        self.tabs[index].favourite = Some(url);
        if self.tabs[index].pin.is_some() {
            self.toggle_pin_without_alert(index, cx);
        }
        self.show_action_alert("Added to Favourites", cx);
        cx.notify();
    }

    /// Removes the favourite at `url`. Its tab returns to the tab list.
    pub(super) fn remove_favourite(
        &mut self,
        url: &str,
        space_id: SpaceId,
        cx: &mut Context<Self>,
    ) {
        let existed = Settings::get(cx)
            .spaces
            .get(space_id)
            .is_some_and(|space| space.favourite_for(url).is_some());
        for tab in &mut self.tabs {
            if tab.space == space_id && tab.favourite.as_deref() == Some(url) {
                tab.favourite = None;
            }
        }
        if self.added_favourite.as_deref() == Some(url) {
            self.added_favourite = None;
        }
        Settings::update(cx, |settings| {
            if let Some(space) = settings.spaces.get_mut(space_id) {
                space.remove_favourite(url);
            }
        });
        if existed {
            self.show_action_alert("Removed from Favourites", cx);
        }
        cx.notify();
    }

    /// The tab that belongs to the favourite at `url`, if it is open.
    pub(super) fn favourite_tab(&self, url: &str, space_id: SpaceId) -> Option<usize> {
        self.tabs
            .iter()
            .position(|tab| tab.space == space_id && tab.favourite.as_deref() == Some(url))
    }

    /// The menu for the favourite at `index`: close its tab while open, or
    /// remove it.
    fn favourite_menu_items(&self, index: usize, cx: &Context<Self>) -> Vec<MenuItem> {
        let space = self.current_space(cx);
        let url = Settings::get(cx)
            .spaces
            .get(space)
            .and_then(|saved| saved.favourites.get(index))
            .map(|favourite| favourite.url.clone())
            .unwrap_or_default();
        let open_tab = self.favourite_tab(&url, space);
        open_tab
            .map(|tab| command_item("Close Tab", MenuCommand::CloseTab(tab)))
            .into_iter()
            .chain([command_item(
                "Remove from Favourites",
                MenuCommand::RemoveFavourite { url, space },
            )])
            .collect()
    }
}

/// The icon a tab shows now, whether awake or asleep.
fn tab_favicon(tab: &BrowserTab, cx: &gpui::App) -> Option<Favicon> {
    match &tab.content {
        TabContent::Web(webview) => webview.read(cx).favicon.clone(),
        TabContent::Sleeping(sleeping) => sleeping.favicon.clone(),
        TabContent::Page(_) => None,
    }
}
