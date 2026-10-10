//! Opening, closing and switching tabs, and the tab strip they show in.

use gpui::{Context, KeyDownEvent, Window};
use photon_core::Shortcut;
use std::time::{Duration, Instant};

use super::super::icons::LOADING_SPINNER_STEPS;
use super::super::pages::{NEW_TAB, PageDefinition};
use super::super::settings::Settings;
use super::super::tabs::{ICON_ENTRANCE, TabIcon};
use super::BrowserWindow;
use super::content::{
    BrowserTab, TabContent, create_webview, create_webview_from_session, follow_appearance,
};
use crate::platform::engine::RequestedWebView;

/// A closed tab's page and position, for reopening.
pub(super) struct ClosedTab {
    index: usize,
    url: String,
}

/// How many closed tabs a window remembers.
const CLOSED_TAB_LIMIT: usize = 25;

/// Time between loading spinner steps.
const SPINNER_STEP_INTERVAL: Duration = Duration::from_millis(80);

impl BrowserWindow {
    pub(super) fn activate_tab(
        &mut self,
        index: usize,
        focus_contents: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.tabs.len() {
            return;
        }
        if index != self.active_tab {
            self.open_menu = None;
            // The find bar searches one page; switching tabs ends its search.
            self.close_find_bar(false, window, cx);
            if let Some(previous) = self.tabs.get_mut(self.active_tab) {
                previous.last_active = Instant::now();
            }
            if let Some(previous) = self
                .tabs
                .get(self.active_tab)
                .and_then(|tab| tab.content.webview())
            {
                previous.update(cx, |view, _| {
                    view.session.set_visible(false);
                    view.session.set_focus(false);
                });
            }
            self.active_tab = index;
        }
        self.tabs[index].last_active = Instant::now();
        // A tab of another space brings its space; a favourite's tab
        // belongs to whichever space it is opened in.
        let current = self.current_space(cx);
        if self.tabs[index].favourite.is_some() {
            self.tabs[index].space = current;
        } else if self.shows_spaces(cx) && self.tabs[index].space != current {
            self.switch_space(self.tabs[index].space, cx);
        }
        let webview = self.tabs[index].content.webview();
        if let Some(webview) = webview.as_ref() {
            let window_visible = self.window_visible;
            webview.update(cx, |view, cx| {
                view.session.set_visible(window_visible);
                if focus_contents {
                    window.focus(&view.focus_handle, cx);
                }
                view.track_engine_focus(window, cx);
            });
        }
        self.omnibox.update(cx, |omnibox, cx| {
            omnibox.set_content(webview, self.tabs[index].content.page_address(), window, cx);
            if focus_contents && self.tabs[index].content.webview().is_none() {
                omnibox.focus(window, cx);
            }
        });
        self.sync_dialog(window, cx);
        cx.notify();
    }

    fn move_tab_focus(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.activate_tab(index, false, window, cx);
        window.focus(&self.tab_focus_handles[index], cx);
    }

    pub(super) fn open_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.insert_page(self.tabs.len(), NEW_TAB, window, cx);
    }

    pub(super) fn insert_tab(
        &mut self,
        index: usize,
        address: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(address) = address {
            let webview = create_webview(cx, self.runtime.clone(), address);
            self.insert_webview(index, webview, None, true, window, cx);
        } else {
            self.insert_page(index, NEW_TAB, window, cx);
        }
    }

    /// Opens one of Photon's pages in a new tab, or switches to the tab
    /// already showing it when the page keeps to one tab.
    pub(super) fn open_page_tab(
        &mut self,
        page: &'static PageDefinition,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let open = page
            .single_tab
            .then(|| {
                self.tabs.iter().position(|tab| {
                    matches!(&tab.content, TabContent::Page(shown) if shown.definition.is(page))
                })
            })
            .flatten();
        match open {
            Some(index) => self.activate_tab(index, true, window, cx),
            None => self.insert_page(self.tabs.len(), page, window, cx),
        }
    }

    /// Opens `address` in a new tab after `opener`'s, as a link opened from
    /// that page does.
    pub(super) fn open_tab_beside(
        &mut self,
        opener: &gpui::Entity<super::super::PhotonWebView>,
        address: &str,
        activate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let index = self
            .tabs
            .iter()
            .position(|tab| tab.content.webview().as_ref() == Some(opener))
            .map_or(self.tabs.len(), |index| index + 1);
        let webview = create_webview(cx, self.runtime.clone(), address);
        self.insert_webview(index, webview, None, activate, window, cx);
    }

    pub(super) fn open_requested_tab(
        &mut self,
        request: RequestedWebView,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let webview = create_webview_from_session(cx, self.runtime.clone(), request.session);
        self.insert_webview(self.tabs.len(), webview, None, request.activate, window, cx);
    }

    /// Inserts a tab showing `webview`, pinned at `pin` or one of today's
    /// tabs. It adopts the window's page size so it lays out and loads even
    /// while in the background.
    pub(super) fn insert_webview(
        &mut self,
        index: usize,
        webview: gpui::Entity<super::super::PhotonWebView>,
        pin: Option<Shortcut>,
        activate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let viewport = self
            .active_webview()
            .and_then(|webview| webview.read(cx).last_viewport);
        follow_appearance(&webview, window, cx);
        webview.update(cx, |view, _| view.adopt_viewport(viewport));
        let content = TabContent::Web(webview);
        match pin {
            Some(pin) => self.insert_tab_at(index, content, Some(pin), activate, window, cx),
            None => self.insert_content(index, content, activate, window, cx),
        }
    }

    /// Inserts a tab showing one of Photon's own pages, and switches to it.
    pub(super) fn insert_page(
        &mut self,
        index: usize,
        page: &'static PageDefinition,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let content = self.new_page(page, cx);
        self.insert_content(index, content, true, window, cx);
    }

    /// Inserts a tab of today's tabs, never among the pinned ones.
    pub(super) fn insert_content(
        &mut self,
        index: usize,
        content: TabContent,
        activate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let index = index.clamp(self.pinned_count(), self.tabs.len());
        self.insert_tab_at(index, content, None, activate, window, cx);
    }

    /// Inserts a tab, pinned at `pin` or one of today's tabs.
    pub(super) fn insert_tab_at(
        &mut self,
        index: usize,
        content: TabContent,
        pin: Option<Shortcut>,
        activate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let index = index.min(self.tabs.len());
        let subscription = self.subscribe_to_content(&content, window, cx);
        let id = self.allocate_tab_id();
        let space = self.current_space(cx);
        self.tabs
            .insert(index, BrowserTab::new(id, content.clone(), pin, space));
        self.tab_subscriptions.insert(index, subscription);
        self.tab_focus_handles
            .insert(index, cx.focus_handle().tab_stop(self.tabs.len() == 1));
        if index <= self.active_tab && self.tabs.len() > 1 {
            // Keep `active_tab` naming the same page until the switch below.
            self.active_tab += 1;
        }
        if activate {
            self.activate_tab(index, true, window, cx);
        } else {
            if let Some(webview) = content.webview() {
                webview.update(cx, |view, _| {
                    view.session.set_visible(false);
                    view.session.set_focus(false);
                });
            }
            cx.notify();
        }
    }

    fn allocate_tab_id(&mut self) -> u64 {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        id
    }

    pub(super) fn reopen_closed_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(closed) = self.closed_tabs.pop() {
            self.insert_tab(closed.index, Some(&closed.url), window, cx);
        }
    }

    /// Activates the tab `offset` places from the active one, wrapping at the ends.
    pub(super) fn select_relative_tab(
        &mut self,
        offset: isize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(index) = self.listed_tab_after(self.active_tab, offset, cx) {
            self.activate_tab(index, true, window, cx);
        }
    }

    pub(super) fn close_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }

        let removed = self.tabs[index].clone();
        if let Some(webview) = removed.content.webview() {
            let favicon = webview.update(cx, |view, _| {
                view.session.set_visible(false);
                view.session.set_focus(false);
                view.favicon.take()
            });
            if let Some(favicon) = favicon {
                cx.drop_image(favicon.image, Some(window));
            }
        }
        self.revealed_icons.borrow_mut().remove(&removed.id);

        if self.tabs.len() == 1 && Settings::get(cx).close_window_with_last_tab {
            window.remove_window();
            return;
        }
        self.remember_closed(index, &removed.content, cx);
        if self.tabs.len() == 1 {
            self.replace_last_tab_with_new_tab(window, cx);
            return;
        }

        let was_active = index == self.active_tab;
        let was_pinned = self.tabs.remove(index).pin.is_some();
        drop(self.tab_subscriptions.remove(index));
        self.tab_focus_handles.remove(index);
        if index < self.active_tab {
            self.active_tab -= 1;
        } else if was_active {
            self.active_tab = index.min(self.tabs.len() - 1);
        }

        if was_pinned {
            self.save_pinned_tabs(cx);
        }
        if was_active {
            self.activate_tab(self.active_tab, true, window, cx);
        } else {
            cx.notify();
        }
    }

    /// Keeps a closed web page so it can be reopened.
    fn remember_closed(&mut self, index: usize, content: &TabContent, cx: &Context<Self>) {
        let Some(webview) = content.webview() else {
            return;
        };
        let view = webview.read(cx);
        if view.has_page() {
            if self.closed_tabs.len() == CLOSED_TAB_LIMIT {
                self.closed_tabs.remove(0);
            }
            self.closed_tabs.push(ClosedTab {
                index,
                url: view.state.url.clone(),
            });
        }
    }

    /// Keeps the window open after its last tab closes, showing a fresh new
    /// tab page in that tab's place.
    fn replace_last_tab_with_new_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_find_bar(false, window, cx);
        let content = self.new_page(NEW_TAB, cx);
        self.tab_subscriptions[0] = self.subscribe_to_content(&content, window, cx);
        let id = self.allocate_tab_id();
        self.tabs[0] = BrowserTab::new(id, content, None, self.current_space(cx));
        self.active_tab = 0;
        self.activate_tab(0, true, window, cx);
    }

    /// Whether a loading tab or a notice shows a spinner.
    fn spinner_shown(&self, cx: &Context<Self>) -> bool {
        self.notice_is_working(cx)
            || self
                .tabs
                .iter()
                .filter_map(|tab| tab.content.webview())
                .any(|webview| {
                    let view = webview.read(cx);
                    view.state.loading && view.has_page()
                })
    }

    /// Steps the spinners while any is shown. With reduced motion they stay still.
    pub(super) fn animate_spinner(&mut self, cx: &mut Context<Self>) {
        if self.spinner_running || cx.reduce_motion() || !self.spinner_shown(cx) {
            return;
        }
        self.spinner_running = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(SPINNER_STEP_INTERVAL).await;
                let keep_running = this
                    .update(cx, |this, cx| {
                        let keep_running = !cx.reduce_motion() && this.spinner_shown(cx);
                        if keep_running {
                            this.spinner_step = (this.spinner_step + 1) % LOADING_SPINNER_STEPS;
                            if this.window_visible {
                                this.chrome.update(cx, |_, cx| cx.notify());
                                if this.notice_is_working(cx) {
                                    cx.notify();
                                }
                            }
                        } else {
                            this.spinner_running = false;
                        }
                        keep_running
                    })
                    .unwrap_or(false);
                if !keep_running {
                    break;
                }
            }
        })
        .detach();
    }

    /// Up and down arrows move between tabs from a focused tab.
    pub(super) fn tab_key_down(
        &mut self,
        index: usize,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.modifiers.modified() {
            return;
        }
        let offset = match event.keystroke.key.as_str() {
            "up" => -1,
            "down" => 1,
            _ => return,
        };
        if let Some(next_index) = self.listed_tab_after(index, offset, cx) {
            self.move_tab_focus(next_index, window, cx);
        }
        window.prevent_default();
        cx.stop_propagation();
    }

    /// Whether a tab's icon is still within its appear animation. A newly
    /// revealed icon starts it; the same icon shown again does not.
    pub(super) fn icon_appearing(&self, tab: u64, icon: &TabIcon) -> bool {
        let Some(revealed) = icon.revealed() else {
            return false;
        };
        let mut revealed_icons = self.revealed_icons.borrow_mut();
        match revealed_icons.get(&tab) {
            Some((icon, since)) if *icon == revealed => {
                since.elapsed() < ICON_ENTRANCE.total_duration()
            }
            _ => {
                revealed_icons.insert(tab, (revealed, Instant::now()));
                true
            }
        }
    }
}
