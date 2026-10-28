//! Pinned tabs, as in Arc: they sit above today's tabs, keep the address
//! they were pinned at, and open again at launch. Today's tabs can be closed
//! once left unused for twelve hours.

use gpui::{Context, Task, Window};
use photon_core::{BrowserState, Shortcut, SpaceId, internal_page_url, should_archive};
use std::time::Duration;

use crate::platform::app_update::update_app;

use super::super::pages::find_page;
use super::super::settings::Settings;
use super::BrowserWindow;
use super::content::{BrowserTab, SleepingTab, TabContent};

/// How often the window looks for tabs left unused.
const ARCHIVE_CHECK_INTERVAL: Duration = Duration::from_secs(5 * 60);

impl BrowserWindow {
    pub(super) fn tab_index_by_id(&self, id: u64) -> Option<usize> {
        self.tabs.iter().position(|tab| tab.id == id)
    }

    /// How many tabs at the start of the window are pinned.
    pub(super) fn pinned_count(&self) -> usize {
        self.tabs.iter().take_while(|tab| tab.pin.is_some()).count()
    }

    /// Pins the tab at `index` at its current address, after the other
    /// pinned tabs, or returns a pinned tab to the top of today's tabs.
    pub(super) fn toggle_pin(&mut self, index: usize, cx: &mut Context<Self>) {
        self.toggle_pin_inner(index, true, cx);
    }

    /// Unpins a tab as part of another action, such as adding it to Favourites.
    pub(super) fn toggle_pin_without_alert(&mut self, index: usize, cx: &mut Context<Self>) {
        self.toggle_pin_inner(index, false, cx);
    }

    fn toggle_pin_inner(&mut self, index: usize, announce: bool, cx: &mut Context<Self>) {
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
        let pinned = pin.is_some();
        let previous = std::mem::replace(&mut self.tabs[index].pin, pin);
        self.save_pin_change(&self.tabs[index], previous, cx);
        self.move_tab_to(index, to, cx);
        if announce {
            self.show_action_alert(if pinned { "Tab pinned" } else { "Tab unpinned" }, cx);
        }
        cx.notify();
    }

    /// Whether the tab at `index` can be pinned: a web page, or one of the
    /// browser's pages that has an address.
    pub(super) fn can_pin(&self, index: usize, cx: &Context<Self>) -> bool {
        self.tabs
            .get(index)
            .is_some_and(|tab| tab.pin.is_some() || pin_for(tab, cx).is_some())
    }

    /// Unpins a tab dropped below the pinned section, placing it first among
    /// today's tabs. An unpinned tab dropped there also moves to the front.
    pub(super) fn drop_tab_into_today(&mut self, from: usize, cx: &mut Context<Self>) {
        match self.tabs.get(from) {
            Some(tab) if tab.pin.is_some() => self.toggle_pin(from, cx),
            Some(_) => {
                let first_today = self.pinned_count();
                self.move_tab_to(from, first_today, cx);
            }
            None => {}
        }
    }

    /// Moves a tab onto another tab, pinning or unpinning it to join that
    /// section while preserving the section's existing ordering rules.
    pub(super) fn drop_tab(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.get(from) else {
            return;
        };
        let pinned = to < self.pinned_count();
        if pinned != tab.pin.is_some() {
            let pin = if pinned { pin_for(tab, cx) } else { None };
            if pinned && pin.is_none() {
                return;
            }
            let previous = std::mem::replace(&mut self.tabs[from].pin, pin);
            self.save_pin_change(&self.tabs[from], previous, cx);
            self.move_tab_to(from, to, cx);
            self.show_action_alert(if pinned { "Tab pinned" } else { "Tab unpinned" }, cx);
            cx.notify();
        } else {
            self.move_tab_to(from, to, cx);
        }
    }

    /// Keeps the tab's new pin in its space's saved pinned tabs, after the
    /// others, or forgets the pin it had. The saved list changes only by the
    /// pin itself, as other windows can hold the space's other pinned tabs.
    fn save_pin_change(
        &self,
        tab: &BrowserTab,
        previous: Option<Shortcut>,
        cx: &mut Context<Self>,
    ) {
        let space = tab.space;
        let added = tab.pin.clone();
        Settings::update(cx, |settings| {
            let Some(saved) = settings.spaces.get_mut(space) else {
                return;
            };
            if let Some(previous) = &previous
                && let Some(position) = saved
                    .pinned_tabs
                    .iter()
                    .position(|pin| pin.url == previous.url)
            {
                saved.pinned_tabs.remove(position);
            }
            if let Some(added) = added
                && !saved.pinned_tabs.iter().any(|pin| pin.url == added.url)
            {
                saved.pinned_tabs.push(added);
            }
        });
    }

    /// Forgets a closed pinned tab's pin in its space's saved pinned tabs.
    pub(super) fn forget_pin(&self, tab: &BrowserTab, cx: &mut Context<Self>) {
        let mut unpinned = tab.clone();
        let previous = unpinned.pin.take();
        self.save_pin_change(&unpinned, previous, cx);
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
                        // Pinned pages are restored in the background. Keep
                        // their saved identity now and initialize the Engine
                        // view only if the user selects one.
                        self.insert_tab_at_immediately(
                            index,
                            TabContent::Sleeping(SleepingTab {
                                state: BrowserState {
                                    url: pin.url.clone(),
                                    title: pin.title.clone(),
                                    ..BrowserState::default()
                                },
                                favicon: None,
                            }),
                            Some(pin),
                            false,
                            window,
                            cx,
                        );
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
        let handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(ARCHIVE_CHECK_INTERVAL).await;
                // Waits while the app is busy; stops once the window is gone.
                let open = update_app(cx, |cx| {
                    handle
                        .update(cx, |_, window, cx| {
                            this.update(cx, |this, cx| this.archive_unused_tabs(window, cx))
                        })
                        .is_ok_and(|updated| updated.is_ok())
                })
                .await;
                if open != Some(true) {
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
                self.active_tab == Some(index),
                tab.last_active.elapsed(),
            );
            if unused {
                self.close_tab(index, window, cx);
            }
        }
    }
}

/// The address and title a tab would be pinned at.
pub(super) fn pin_for(tab: &BrowserTab, cx: &gpui::App) -> Option<Shortcut> {
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
        TabContent::Sleeping(sleeping) => (!sleeping.state.url.is_empty()).then(|| Shortcut {
            title: sleeping.state.title.clone(),
            url: sleeping.state.url.clone(),
        }),
    }
}
