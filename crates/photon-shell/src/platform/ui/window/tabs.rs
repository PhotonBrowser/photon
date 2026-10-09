//! Opening, closing and switching tabs, and the tab strip they show in.

use gpui::{Context, EntityId, KeyDownEvent, Window, prelude::*};
use std::time::{Duration, Instant};

use super::super::icons::LOADING_SPINNER_STEPS;
use super::super::tabs::{ICON_APPEAR_DURATION, TabIcon, TabItem, tab_strip};
use super::{BrowserWindow, create_webview};

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
            if let Some(previous) = self.tabs.get(self.active_tab) {
                previous.update(cx, |view, _| {
                    view.session.set_visible(false);
                    view.session.set_focus(false);
                });
            }
            self.active_tab = index;
        }
        let webview = self.tabs[index].clone();
        let is_blank_tab = webview.read(cx).is_blank_tab();
        let window_visible = self.window_visible;
        webview.update(cx, |view, cx| {
            view.session.set_visible(window_visible);
            if focus_contents && !is_blank_tab {
                window.focus(&view.focus_handle, cx);
            }
            view.track_engine_focus(window, cx);
        });
        self.omnibox.update(cx, |omnibox, cx| {
            omnibox.set_webview(webview, window, cx);
            if focus_contents && is_blank_tab {
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
        self.insert_tab(self.tabs.len(), None, window, cx);
    }

    fn insert_tab(
        &mut self,
        index: usize,
        address: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let webview = create_webview(
            cx,
            self.runtime.clone(),
            self.theme.clone(),
            address,
            address.is_none(),
        );
        let index = index.min(self.tabs.len());
        self.tab_subscriptions
            .insert(index, Self::subscribe_to_tab(&webview, window, cx));
        self.tabs.insert(index, webview);
        self.tab_focus_handles
            .insert(index, cx.focus_handle().tab_stop(false));
        if index <= self.active_tab {
            // Keep `active_tab` naming the same page until the switch below.
            self.active_tab += 1;
        }
        self.activate_tab(index, true, window, cx);
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
        let count = self.tabs.len() as isize;
        let index = (self.active_tab as isize + offset).rem_euclid(count) as usize;
        self.activate_tab(index, true, window, cx);
    }

    pub(super) fn close_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }

        let removed = self.tabs[index].clone();
        let favicon = removed.update(cx, |view, _| {
            view.session.set_visible(false);
            view.session.set_focus(false);
            view.favicon.take()
        });
        if let Some(favicon) = favicon {
            cx.drop_image(favicon.image, Some(window));
        }
        self.revealed_icons
            .borrow_mut()
            .remove(&removed.entity_id());

        if self.tabs.len() == 1 {
            window.remove_window();
            return;
        }

        let view = removed.read(cx);
        if view.has_page() {
            let url = view.state.url.clone();
            if self.closed_tabs.len() == CLOSED_TAB_LIMIT {
                self.closed_tabs.remove(0);
            }
            self.closed_tabs.push(ClosedTab { index, url });
        }

        let was_active = index == self.active_tab;
        self.tabs.remove(index);
        drop(self.tab_subscriptions.remove(index));
        self.tab_focus_handles.remove(index);
        if index < self.active_tab {
            self.active_tab -= 1;
        } else if was_active {
            self.active_tab = index.min(self.tabs.len() - 1);
        }

        if was_active {
            self.activate_tab(self.active_tab, true, window, cx);
        } else {
            cx.notify();
        }
    }

    /// Whether a loading tab or a notice shows a spinner.
    fn spinner_shown(&self, cx: &Context<Self>) -> bool {
        self.notice_is_working(cx)
            || self.tabs.iter().any(|tab| {
                let view = tab.read(cx);
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

    pub(super) fn tab_strip(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tabs = self
            .tabs
            .iter()
            .enumerate()
            .map(|(index, webview)| {
                let view = webview.read(cx);
                let is_blank = !view.has_page();
                let icon = if is_blank {
                    TabIcon::NewTab
                } else if view.shows_spinner() {
                    TabIcon::Loading(self.spinner_step)
                } else if let Some(favicon) = view.favicon.clone() {
                    TabIcon::Favicon(favicon)
                } else {
                    TabIcon::Page
                };
                let label = if is_blank {
                    "New Tab".into()
                } else if !view.state.title.trim().is_empty() {
                    view.state.title.clone()
                } else {
                    view.state.url.clone()
                };
                TabItem {
                    id: format!("browser-tab-{:?}", webview.entity_id()),
                    label,
                    icon_appearing: self.icon_appearing(webview.entity_id(), &icon),
                    icon,
                    active: index == self.active_tab,
                    focus_handle: self.tab_focus_handles[index].clone(),
                    on_select: Box::new(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.activate_tab(index, true, window, cx);
                    })),
                    on_key_down: Box::new(cx.listener(
                        move |this, event: &KeyDownEvent, window, cx| {
                            this.tab_key_down(index, event, window, cx);
                        },
                    )),
                    on_close: Box::new(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.close_tab(index, window, cx);
                    })),
                }
            })
            .collect();

        tab_strip(
            tabs,
            Box::new(cx.listener(|this, _, window, cx| {
                cx.stop_propagation();
                this.open_tab(window, cx);
            })),
            self.palette(window),
        )
    }

    /// Left and right arrows move between tabs from a focused tab.
    fn tab_key_down(
        &mut self,
        index: usize,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.modifiers.modified() {
            return;
        }
        let count = self.tabs.len();
        let next_index = match event.keystroke.key.as_str() {
            "left" => (index + count - 1) % count,
            "right" => (index + 1) % count,
            _ => return,
        };
        self.move_tab_focus(next_index, window, cx);
        window.prevent_default();
        cx.stop_propagation();
    }

    /// Whether a tab's icon is still within its appear animation. A newly
    /// revealed icon starts it; the same icon shown again does not.
    fn icon_appearing(&self, tab: EntityId, icon: &TabIcon) -> bool {
        let Some(revealed) = icon.revealed() else {
            return false;
        };
        let mut revealed_icons = self.revealed_icons.borrow_mut();
        match revealed_icons.get(&tab) {
            Some((icon, since)) if *icon == revealed => since.elapsed() < ICON_APPEAR_DURATION,
            _ => {
                revealed_icons.insert(tab, (revealed, Instant::now()));
                true
            }
        }
    }
}
