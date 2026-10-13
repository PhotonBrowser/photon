//! Schedule quiet inactive Engine views to sleep at their deadlines.

use futures::channel::mpsc::{Sender, channel};
use futures::future::{Either, select};
use futures::{FutureExt, StreamExt, pin_mut};
use gpui::{Context, Task, Window};
use photon_core::TabSleepMode;
use std::time::{Duration, Instant};

use crate::platform::app_update::update_app;

use super::super::super::trace;
use super::super::settings::Settings;
use super::BrowserWindow;
use super::content::{BrowserTab, SleepingTab, TabContent};

/// The activity monitor samples page quietness at this interval.
/// Allow two samples after the sleep timeout before putting a tab to sleep.
const SLEEP_CHECK_INTERVAL: Duration = Duration::from_millis(500);

pub(super) type TabSleepWaker = Sender<()>;

impl BrowserWindow {
    pub(super) fn sleep_inactive_tabs_regularly(
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (TabSleepWaker, Task<()>) {
        let handle = window.window_handle();
        let (waker, mut wakeups) = channel(1);
        let task = cx.spawn(async move |this, cx| {
            loop {
                let delay = update_app(cx, |cx| {
                    this.read_with(cx, |this, app| this.next_sleep_check_delay(app))
                })
                .await;
                let Some(Ok(delay)) = delay else {
                    break;
                };

                let deadline_elapsed = if let Some(delay) = delay {
                    let timer = cx.background_executor().timer(delay).fuse();
                    let wakeup = wakeups.next().fuse();
                    pin_mut!(timer, wakeup);
                    match select(timer, wakeup).await {
                        Either::Left(((), _)) => true,
                        Either::Right((Some(()), _)) => false,
                        Either::Right((None, _)) => break,
                    }
                } else if wakeups.next().await.is_some() {
                    false
                } else {
                    break;
                };

                if deadline_elapsed {
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
            }
        });
        (waker, task)
    }

    pub(super) fn wake_tab_sleep_scheduler(&mut self) {
        let _ = self.tab_sleep_waker.try_send(());
    }

    fn next_sleep_check_delay(&self, cx: &gpui::App) -> Option<Duration> {
        let mode = Settings::get(cx).tab_sleep_mode;
        if mode == TabSleepMode::Off {
            return None;
        }
        let eligible = self
            .tabs
            .iter()
            .enumerate()
            .filter(|(index, tab)| self.is_sleep_candidate(*index, tab, cx))
            .count();
        let timeout = photon_tab_sleep::idle_timeout(mode, eligible)?;
        let activity_quiet_for = timeout + SLEEP_CHECK_INTERVAL * 2;
        let now = Instant::now();
        self.tabs
            .iter()
            .enumerate()
            .filter_map(|(index, tab)| {
                if !self.is_sleep_candidate(index, tab, cx) {
                    return None;
                }
                let TabContent::Web(webview) = &tab.content else {
                    return None;
                };
                let idle_for = webview.read(cx).current_activity_idle_for()?;
                let active_delay = (tab.last_active + timeout).saturating_duration_since(now);
                let quiet_delay = activity_quiet_for.saturating_sub(idle_for);
                Some(active_delay.max(quiet_delay))
            })
            .min()
    }

    pub(super) fn sleep_idle_tabs(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
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

    pub(super) fn sync_activity_monitors(&mut self, mode: TabSleepMode, cx: &mut Context<Self>) {
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
