//! Periodically mark quiet inactive Engine views as sleeping.

use gpui::{Context, Task, Window};
use photon_core::TabSleepMode;
use std::time::Duration;

use crate::platform::app_update::update_app;

use super::super::super::trace;
use super::super::settings::Settings;
use super::BrowserWindow;
use super::content::{BrowserTab, SleepingTab, TabContent};

/// Keeps Maximum mode close to its five-second deadline without polling on
/// every frame. The sweep itself only reads the current tab list.
const SLEEP_CHECK_INTERVAL: Duration = Duration::from_millis(500);

impl BrowserWindow {
    pub(super) fn sleep_inactive_tabs_regularly(
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        let handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(SLEEP_CHECK_INTERVAL).await;
                let open = update_app(cx, |cx| {
                    handle
                        .update(cx, |_, window, cx| {
                            this.update(cx, |this, cx| this.sleep_idle_tabs(window, cx))
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

    fn sleep_idle_tabs(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let mode = Settings::get(cx).tab_sleep_mode;
        self.sync_activity_monitors(mode, cx);
        if mode == TabSleepMode::Off {
            let mut woke = 0;
            for tab in &mut self.tabs {
                if let TabContent::Sleeping(sleeping) = &tab.content {
                    tab.content = TabContent::Web(sleeping.webview.clone());
                    woke += 1;
                }
            }
            if woke > 0 {
                cx.notify();
            }
            return;
        }
        let eligible = self
            .tabs
            .iter()
            .enumerate()
            .filter(|(index, tab)| self.is_sleep_candidate(*index, tab, cx))
            .count();
        let Some(timeout) = photon_tab_sleep::idle_timeout(mode, eligible) else {
            return;
        };

        let mut changed = 0;
        for index in (0..self.tabs.len()).rev() {
            let tab = &self.tabs[index];
            let Some(webview) = tab.content.webview() else {
                continue;
            };
            let candidate = self.is_sleep_candidate(index, tab, cx);
            let sleeping = matches!(&tab.content, TabContent::Sleeping(_));
            if !candidate {
                if sleeping {
                    self.tabs[index].content = TabContent::Web(webview);
                    changed += 1;
                }
                continue;
            }
            if sleeping || tab.last_active.elapsed() < timeout {
                continue;
            }
            // Engine activity is sampled on its own 500 ms timer. Require two
            // more sweep intervals of quiet so work near the sleep deadline
            // can be reported before the tab is marked asleep.
            let activity_quiet_for = timeout + SLEEP_CHECK_INTERVAL * 2;
            if !webview
                .read(cx)
                .current_activity_idle_for()
                .is_some_and(|idle_for| idle_for >= activity_quiet_for)
            {
                continue;
            }
            let safe_to_sleep = webview.update(cx, |view, _| {
                if !view.has_page()
                    || view.state.loading
                    || view.audio_playing
                    || view.page_unresponsive
                    || view.dialogs.open().is_some()
                    || view.activity_status != Some(false)
                    || !view
                        .current_activity_idle_for()
                        .is_some_and(|idle_for| idle_for >= activity_quiet_for)
                {
                    return false;
                }
                view.session.set_visible(false);
                view.session.set_focus(false);
                true
            });
            if safe_to_sleep {
                self.tabs[index].content = TabContent::Sleeping(SleepingTab { webview });
                changed += 1;
            }
        }
        if changed > 0 {
            trace(format_args!(
                "updated {changed} inactive tab(s) in the sleep policy"
            ));
            cx.notify();
        }
    }

    fn sync_activity_monitors(&mut self, mode: TabSleepMode, cx: &mut Context<Self>) {
        for (index, tab) in self.tabs.iter().enumerate() {
            let enabled = mode != TabSleepMode::Off
                && self.active_tab != Some(index)
                && tab.pin.is_none()
                && tab.favourite.is_none();
            if let Some(webview) = tab.content.webview()
                && webview.read(cx).activity_monitor_enabled() != enabled
            {
                webview.update(cx, |view, _| view.set_activity_monitor_enabled(enabled));
            }
        }
    }

    fn is_sleep_candidate(&self, index: usize, tab: &BrowserTab, cx: &Context<Self>) -> bool {
        if self.active_tab == Some(index)
            || self.closing_tabs.contains(&tab.id)
            || tab.pin.is_some()
            || tab.favourite.is_some()
        {
            return false;
        }
        let Some(webview) = tab.content.webview() else {
            return false;
        };
        let view = webview.read(cx);
        view.has_page()
            && !view.state.loading
            && !view.audio_playing
            && !view.page_unresponsive
            && view.dialogs.open().is_none()
            && view.activity_status == Some(false)
    }
}
