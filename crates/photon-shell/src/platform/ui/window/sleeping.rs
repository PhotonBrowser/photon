//! Stop rendering inactive Engine views after their sleep deadlines.

use futures::channel::mpsc::{Sender, channel};
use futures::future::{Either, select};
use futures::{FutureExt, StreamExt, pin_mut};
use gpui::{Context, Task, Window};
use std::time::{Duration, Instant};

use crate::platform::app_update::update_app;

use super::super::super::trace;
use super::super::settings::Settings;
use super::BrowserWindow;
use super::content::{BrowserTab, SleepingTab, TabContent};

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
        let timeout = photon_tab_sleep::idle_timeout(Settings::get(cx).tab_sleep_mode)?;
        let now = Instant::now();
        self.tabs
            .iter()
            .enumerate()
            .filter(|(index, tab)| self.is_sleep_candidate(*index, tab))
            .filter_map(|(_, tab)| {
                let TabContent::Web(webview) = &tab.content else {
                    return None;
                };
                let view = webview.read(cx);
                let safe_to_sleep = view.has_page()
                    && !view.state.loading
                    && !view.audio_playing
                    && !view.page_unresponsive
                    && view.dialogs.open().is_none();
                safe_to_sleep.then(|| (tab.last_active + timeout).saturating_duration_since(now))
            })
            .min()
    }

    pub(super) fn sleep_idle_tabs(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let mode = Settings::get(cx).tab_sleep_mode;
        let Some(timeout) = photon_tab_sleep::idle_timeout(mode) else {
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
        };

        let mut changed = 0;
        for index in (0..self.tabs.len()).rev() {
            let tab = &self.tabs[index];
            let Some(webview) = tab.content.webview() else {
                continue;
            };
            let candidate = self.is_sleep_candidate(index, tab);
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
            let safe_to_sleep = webview.update(cx, |view, _| {
                if !view.has_page()
                    || view.state.loading
                    || view.audio_playing
                    || view.page_unresponsive
                    || view.dialogs.open().is_some()
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

    fn is_sleep_candidate(&self, index: usize, tab: &BrowserTab) -> bool {
        self.active_tab != Some(index) && !self.closing_tabs.contains(&tab.id)
    }
}
