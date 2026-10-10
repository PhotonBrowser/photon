//! Pinned tabs, as in Arc: they sit above today's tabs, keep the address
//! they were pinned at, and open again at launch. Today's tabs can be closed
//! once left unused for twelve hours.

use gpui::{Context, Task, Window};
use photon_core::{Shortcut, SpaceId, internal_page_url, pinned_after_drop, should_archive};
use std::time::Duration;

use super::super::pages::find_page;
use super::super::settings::Settings;
use super::BrowserWindow;
use super::content::{BrowserTab, TabContent, create_webview};

/// How often the window looks for tabs left unused.
const ARCHIVE_CHECK_INTERVAL: Duration = Duration::from_secs(5 * 60);

impl BrowserWindow {
    /// How many tabs at the start of the window are pinned.
    pub(super) fn pinned_count(&self) -> usize {
        self.tabs.iter().take_while(|tab| tab.pin.is_some()).count()
    }

    /// Pins the tab at `index` at its current address, after the other
    /// pinned tabs, or returns a pinned tab to the top of today's tabs.
    pub(super) fn toggle_pin(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(index) else {
            return;
        };
        let pin = match tab.pin {
            Some(_) => None,
            None => match pin_for(tab, cx) {
                Some(pin) => Some(pin),
                None => return,
            },
        };
        let to = match pin {
            Some(_) => self.pinned_count(),
            None => self.pinned_count() - 1,
        };
        self.tabs[index].pin = pin;
        self.move_tab_to(index, to, cx);
        self.save_pinned_tabs(cx);
        cx.notify();
    }

    /// Whether the tab at `index` can be pinned: a web page, or one of the
    /// browser's pages that has an address.
    pub(super) fn can_pin(&self, index: usize, cx: &Context<Self>) -> bool {
        self.tabs
            .get(index)
            .is_some_and(|tab| tab.pin.is_some() || pin_for(tab, cx).is_some())
    }

    /// Moves the tab at `from` onto the tab at `to`, joining that tab's
    /// section: dropped among the pinned tabs it is pinned, among today's
    /// it is not.
    pub(super) fn drop_tab(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(from) else {
            return;
        };
        let pinned = pinned_after_drop(to, self.pinned_count());
        if pinned != tab.pin.is_some() {
            let pin = if pinned { pin_for(tab, cx) } else { None };
            if pinned && pin.is_none() {
                return;
            }
            self.tabs[from].pin = pin;
            self.move_tab_to(from, to, cx);
            self.save_pinned_tabs(cx);
            cx.notify();
        } else {
            self.move_tab_to(from, to, cx);
        }
    }

    /// Unpins a tab dropped below the pinned tabs, making it the first of
    /// today's.
    pub(super) fn drop_tab_into_today(&mut self, from: usize, cx: &mut Context<Self>) {
        match self.tabs.get(from) {
            Some(tab) if tab.pin.is_some() => self.toggle_pin(from, cx),
            Some(_) => {
                let first = self.pinned_count();
                self.move_tab_to(from, first, cx);
            }
            None => {}
        }
    }

    /// Saves each space's pinned tabs, so they open again at launch.
    pub(super) fn save_pinned_tabs(&self, cx: &mut Context<Self>) {
        let pinned_in = |space: SpaceId| -> Vec<Shortcut> {
            self.tabs
                .iter()
                .filter(|tab| tab.space == space)
                .filter_map(|tab| tab.pin.clone())
                .collect()
        };
        let changed = Settings::get(cx)
            .spaces
            .all()
            .iter()
            .any(|space| space.pinned_tabs != pinned_in(space.id));
        if changed {
            Settings::update(cx, |settings| {
                let ids: Vec<SpaceId> =
                    settings.spaces.all().iter().map(|space| space.id).collect();
                for id in ids {
                    if let Some(space) = settings.spaces.get_mut(id) {
                        space.pinned_tabs = pinned_in(id);
                    }
                }
            });
        }
    }

    /// Opens every space's saved pinned tabs at the start of the window, in
    /// the background.
    pub(super) fn restore_pinned_tabs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let spaces: Vec<(SpaceId, Vec<Shortcut>)> = Settings::get(cx)
            .spaces
            .all()
            .iter()
            .map(|space| (space.id, space.pinned_tabs.clone()))
            .collect();
        for (space, pinned) in spaces {
            for pin in pinned {
                let index = self.pinned_count();
                match find_page(&pin.url) {
                    Some(page) => {
                        let content = self.new_page(page, cx);
                        self.insert_tab_at(index, content, Some(pin), false, window, cx);
                    }
                    None => {
                        let webview = create_webview(cx, self.runtime.clone(), &pin.url);
                        self.insert_webview(index, webview, Some(pin), false, window, cx);
                    }
                }
                self.tabs[index].space = space;
            }
        }
    }

    /// Checks now and then for today's tabs left unused, closing them when
    /// the setting asks. They can be reopened like any closed tab.
    pub(super) fn archive_unused_tabs_regularly(
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(ARCHIVE_CHECK_INTERVAL).await;
                let closed = this.update_in(cx, |this, window, cx| {
                    this.archive_unused_tabs(window, cx);
                });
                if closed.is_err() {
                    break;
                }
            }
        })
    }

    fn archive_unused_tabs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let enabled = Settings::get(cx).archive_tabs;
        for index in (0..self.tabs.len()).rev() {
            let tab = &self.tabs[index];
            let unused = should_archive(
                enabled,
                tab.pin.is_some(),
                index == self.active_tab,
                tab.last_active.elapsed(),
            );
            if unused {
                self.close_tab(index, window, cx);
            }
        }
    }
}

/// The address and title a tab would be pinned at.
fn pin_for(tab: &BrowserTab, cx: &gpui::App) -> Option<Shortcut> {
    match &tab.content {
        TabContent::Web(webview) => {
            let view = webview.read(cx);
            view.has_page().then(|| Shortcut {
                title: view.state.title.clone(),
                url: view.state.url.clone(),
            })
        }
        TabContent::Page(page) => page.definition.shows_address.then(|| Shortcut {
            title: page.definition.title.to_owned(),
            url: internal_page_url(page.definition.name),
        }),
    }
}
