//! Sleep out-of-view Engine views and release hidden-page memory under pressure.

use futures::channel::mpsc::{Sender, channel};
use futures::future::{Either, select};
use futures::{FutureExt, StreamExt, pin_mut};
use gpui::{App, Context, Global, Task, Window};
use photon_core::MemoryPressure;
use std::collections::HashSet;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::platform::{app_update::update_app, memory_pressure};

use super::super::super::trace;
use super::super::settings::Settings;
use super::BrowserWindow;
use super::content::{SleepingTab, TabContent};
use super::registry::BrowserWindowRegistry;

const MEMORY_PRESSURE_RETRY_INTERVAL: Duration = Duration::from_secs(1);
const ACTIVE_IDLE_RECHECK_INTERVAL: Duration = Duration::from_secs(30);

pub(super) type TabSleepWaker = Sender<SleepWake>;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum SleepWake {
    Recheck,
    MemoryPressure,
    MemoryPressureRetry,
    Deadline,
}

/// Wakes the tab-sleep scheduler whenever the system pressure level changes.
pub(super) struct MemoryPressureMonitor(#[allow(dead_code)] memory_pressure::Monitor);

impl MemoryPressureMonitor {
    pub(super) fn new(waker: TabSleepWaker) -> Self {
        let waker = Mutex::new(waker);
        Self(memory_pressure::Monitor::new(move || {
            let _ = waker
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .try_send(SleepWake::MemoryPressure);
        }))
    }
}

#[cfg(unix)]
#[derive(Default)]
struct MemoryPressureOverride(Option<MemoryPressure>);

#[cfg(unix)]
impl Global for MemoryPressureOverride {}

/// Returns an injected level for isolated automation sessions, otherwise
/// reading the operating system's current pressure state.
pub(super) fn current_memory_pressure(cx: &App) -> MemoryPressure {
    #[cfg(unix)]
    if cx.has_global::<MemoryPressureOverride>()
        && let Some(pressure) = cx.global::<MemoryPressureOverride>().0
    {
        return pressure;
    }
    #[cfg(not(unix))]
    let _ = cx;

    memory_pressure::current()
}

/// Allows the isolated automation harness to exercise pressure cleanup without
/// creating artificial machine-wide memory pressure.
#[cfg(unix)]
pub(super) fn set_test_memory_pressure(
    pressure: Option<MemoryPressure>,
    cx: &mut App,
) -> Option<Instant> {
    if !cx.has_global::<MemoryPressureOverride>() {
        cx.set_global(MemoryPressureOverride::default());
    }
    cx.global_mut::<MemoryPressureOverride>().0 = pressure;

    let retry_at = relieve_memory_pressure(cx);
    for handle in BrowserWindowRegistry::ordered_handles(cx) {
        let Some(window) = handle.downcast::<BrowserWindow>() else {
            continue;
        };
        let _ = window.update(cx, |browser, _, _| browser.wake_tab_sleep_scheduler());
    }
    retry_at
}

#[derive(Default)]
struct MemoryPressureCycle {
    pressure: MemoryPressure,
    cleaned_tabs: HashSet<(u64, u64)>,
    retry_at: Option<Instant>,
}

struct PressureCandidate {
    window_id: u64,
    tab_id: u64,
    memory_bytes: u64,
    last_active: Instant,
}

impl Global for MemoryPressureCycle {}

fn relieve_memory_pressure(cx: &mut App) -> Option<Instant> {
    let pressure = current_memory_pressure(cx);
    if !cx.has_global::<MemoryPressureCycle>() {
        cx.set_global(MemoryPressureCycle::default());
    }

    let handles = BrowserWindowRegistry::ordered_handles(cx);
    for handle in &handles {
        let Some(window) = handle.downcast::<BrowserWindow>() else {
            continue;
        };
        if window
            .update(cx, |browser, _, _| {
                browser
                    .runtime
                    .set_memory_pressure(pressure != MemoryPressure::Normal);
            })
            .is_ok()
        {
            break;
        }
    }

    let now = Instant::now();
    let cleaned_tabs = {
        let cycle = cx.global_mut::<MemoryPressureCycle>();
        if cycle.pressure != pressure {
            cycle.pressure = pressure;
            cycle.cleaned_tabs.clear();
            cycle.retry_at = None;
        }
        if pressure == MemoryPressure::Normal {
            cycle.retry_at = None;
            cycle.cleaned_tabs.clear();
            return None;
        }
        if let Some(retry_at) = cycle.retry_at
            && retry_at > now
        {
            return Some(retry_at);
        }
        cycle.retry_at = None;
        cycle.cleaned_tabs.clone()
    };

    let mut candidates = Vec::new();
    for handle in &handles {
        let Some(window) = handle.downcast::<BrowserWindow>() else {
            continue;
        };
        let window_id = window.window_id().as_u64();
        let Ok(window_candidates) =
            window.update(cx, |browser, _, cx| browser.pressure_candidates(cx))
        else {
            continue;
        };
        candidates.extend(window_candidates.into_iter().filter_map(
            |(tab_id, memory_bytes, last_active)| {
                (!cleaned_tabs.contains(&(window_id, tab_id))).then_some(PressureCandidate {
                    window_id,
                    tab_id,
                    memory_bytes,
                    last_active,
                })
            },
        ));
    }
    candidates.sort_by_key(|candidate| {
        (
            std::cmp::Reverse(candidate.memory_bytes),
            candidate.last_active,
            candidate.window_id,
            candidate.tab_id,
        )
    });

    let batch_size = pressure.cleanup_batch();
    let more_candidates = candidates.len() > batch_size;
    let mut attempted = Vec::new();
    for candidate in candidates.into_iter().take(batch_size) {
        let Some(window) = handles
            .iter()
            .find(|handle| handle.window_id().as_u64() == candidate.window_id)
            .and_then(|handle| handle.downcast::<BrowserWindow>())
        else {
            continue;
        };
        attempted.push((candidate.window_id, candidate.tab_id));
        let _ = window.update(cx, |browser, window, cx| {
            browser.release_tab_memory(
                candidate.tab_id,
                pressure.should_discard_hidden_tabs(),
                window,
                cx,
            )
        });
    }

    let retry_at = if more_candidates && !attempted.is_empty() {
        Some(now + MEMORY_PRESSURE_RETRY_INTERVAL)
    } else {
        None
    };
    if !attempted.is_empty() {
        let cycle = cx.global_mut::<MemoryPressureCycle>();
        cycle.cleaned_tabs.extend(attempted);
        cycle.retry_at = retry_at;
    }
    retry_at
}

impl BrowserWindow {
    pub(super) fn sleep_inactive_tabs_regularly(
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (TabSleepWaker, Task<()>) {
        let handle = window.window_handle();
        let (waker, mut wakeups) = channel(4);
        let task = cx.spawn(async move |this, cx| {
            let mut next_pressure_retry = None;
            loop {
                let sleep_delay = update_app(cx, |cx| {
                    this.read_with(cx, |this, app| this.next_sleep_check_delay(app))
                })
                .await;
                let Some(Ok(sleep_delay)) = sleep_delay else {
                    break;
                };

                let pressure_delay = next_pressure_retry
                    .map(|retry_at: Instant| retry_at.saturating_duration_since(Instant::now()));
                let (delay, timer_wakeup) = match (sleep_delay, pressure_delay) {
                    (Some(sleep), Some(pressure)) if pressure <= sleep => {
                        (Some(pressure), SleepWake::MemoryPressureRetry)
                    }
                    (Some(sleep), Some(_)) | (Some(sleep), None) => {
                        (Some(sleep), SleepWake::Deadline)
                    }
                    (None, Some(pressure)) => (Some(pressure), SleepWake::MemoryPressureRetry),
                    (None, None) => (None, SleepWake::Deadline),
                };

                let work = if let Some(delay) = delay {
                    let timer = cx.background_executor().timer(delay).fuse();
                    let wakeup = wakeups.next().fuse();
                    pin_mut!(timer, wakeup);
                    match select(timer, wakeup).await {
                        Either::Left(((), _)) => Some(timer_wakeup),
                        Either::Right((Some(wakeup), _)) => Some(wakeup),
                        Either::Right((None, _)) => break,
                    }
                } else {
                    match wakeups.next().await {
                        Some(wakeup) => Some(wakeup),
                        None => break,
                    }
                };

                match work {
                    Some(SleepWake::Recheck) => {
                        let Some(retry_at) = update_app(cx, |cx| {
                            if current_memory_pressure(cx) == MemoryPressure::Normal {
                                None
                            } else {
                                relieve_memory_pressure(cx)
                            }
                        })
                        .await
                        else {
                            break;
                        };
                        next_pressure_retry = retry_at;
                    }
                    Some(SleepWake::MemoryPressure) => {
                        let Some(retry_at) = update_app(cx, relieve_memory_pressure).await else {
                            break;
                        };
                        next_pressure_retry = retry_at;
                    }
                    Some(SleepWake::MemoryPressureRetry) => {
                        let Some(retry_at) = update_app(cx, relieve_memory_pressure).await else {
                            break;
                        };
                        next_pressure_retry = retry_at;
                    }
                    Some(SleepWake::Deadline) => {
                        let Some(retry_at) = update_app(cx, relieve_memory_pressure).await else {
                            break;
                        };
                        next_pressure_retry = retry_at;
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
                    None => break,
                }
            }
        });
        (waker, task)
    }

    pub(super) fn wake_tab_sleep_scheduler(&mut self) {
        let _ = self.tab_sleep_waker.try_send(SleepWake::Recheck);
    }

    fn next_sleep_check_delay(&self, cx: &gpui::App) -> Option<Duration> {
        let mode = Settings::get(cx).tab_sleep_mode;
        photon_core::idle_timeout(mode)?;
        let now = Instant::now();
        let system_idle = crate::platform::system_idle_duration();
        self.tabs
            .iter()
            .enumerate()
            .filter_map(|(index, tab)| {
                let TabContent::Web(webview) = &tab.content else {
                    return None;
                };
                let view = webview.read(cx);
                let safe_to_sleep = view.has_page()
                    && !view.state.loading
                    && !view.audio_playing
                    && !view.page_unresponsive
                    && !view.session.needs_beforeunload_check()
                    && view.dialogs.open().is_none();
                if !safe_to_sleep {
                    return None;
                }
                let delay = photon_core::idle_discard_delay(
                    mode,
                    self.is_active_visible_tab(index),
                    now.saturating_duration_since(tab.last_active),
                    system_idle,
                )?;
                Some(if self.is_active_visible_tab(index) {
                    delay.min(ACTIVE_IDLE_RECHECK_INTERVAL)
                } else {
                    delay
                })
            })
            .min()
    }

    pub(super) fn sleep_idle_tabs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mode = Settings::get(cx).tab_sleep_mode;
        if photon_core::idle_timeout(mode).is_none() {
            return;
        }
        let now = Instant::now();
        let system_idle = crate::platform::system_idle_duration();

        let mut changed = 0;
        for index in (0..self.tabs.len()).rev() {
            let tab = &self.tabs[index];
            if !matches!(&tab.content, TabContent::Web(_)) {
                continue;
            }
            if photon_core::idle_discard_delay(
                mode,
                self.is_active_visible_tab(index),
                now.saturating_duration_since(tab.last_active),
                system_idle,
            ) != Some(Duration::ZERO)
            {
                continue;
            }
            if self.discard_tab(index, window, cx) {
                changed += 1;
            }
        }
        if changed > 0 {
            trace(format_args!(
                "discarded {changed} idle page(s) in the sleep policy"
            ));
            cx.notify();
        }
    }

    fn pressure_candidates(&self, cx: &App) -> Vec<(u64, u64, Instant)> {
        self.tabs
            .iter()
            .enumerate()
            .filter(|(index, _)| self.is_pressure_candidate(*index))
            .filter_map(|(_, tab)| {
                let webview = tab.content.webview()?;
                let view = webview.read(cx);
                let safe = view.has_page()
                    && !view.state.loading
                    && !view.audio_playing
                    && !view.page_unresponsive
                    && !view.session.needs_beforeunload_check()
                    && view.dialogs.open().is_none();
                safe.then(|| {
                    (
                        tab.id,
                        view.diagnostics
                            .memory_bytes
                            .or(view.diagnostics.managed_heap_bytes)
                            .unwrap_or_default(),
                        tab.last_active,
                    )
                })
            })
            .collect()
    }

    fn release_tab_memory(
        &mut self,
        id: u64,
        discard_if_safe: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(index) = self.tabs.iter().position(|tab| tab.id == id) else {
            return false;
        };
        if !self.is_pressure_candidate(index) {
            return false;
        }
        if discard_if_safe && self.is_sleep_candidate(index) && self.discard_tab(index, window, cx)
        {
            cx.notify();
            return true;
        }
        let Some(webview) = self.tabs[index].content.webview() else {
            return false;
        };
        webview.update(cx, |view, _| {
            if !view.has_page()
                || view.state.loading
                || view.audio_playing
                || view.page_unresponsive
                || view.dialogs.open().is_some()
            {
                return false;
            }
            view.session.release_hidden_memory();
            true
        })
    }

    fn discard_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(webview) = self.tabs.get(index).and_then(|tab| tab.content.webview()) else {
            return false;
        };
        let had_focus = webview.read(cx).focus_handle.is_focused(window);
        let snapshot = webview.update(cx, |view, _| {
            if !view.has_page()
                || view.state.loading
                || view.audio_playing
                || view.page_unresponsive
                || view.session.needs_beforeunload_check()
                || view.dialogs.open().is_some()
            {
                return None;
            }
            view.set_page_visible(false);
            view.session.set_focus(false);
            Some(SleepingTab {
                state: view.state.clone(),
                favicon: view.favicon.clone(),
            })
        });
        let Some(snapshot) = snapshot else {
            return false;
        };
        if had_focus {
            window.focus(&self.root_focus, cx);
        }
        self.tabs[index].content = TabContent::Sleeping(snapshot);
        self.tab_subscriptions[index] = None;
        true
    }

    fn is_active_visible_tab(&self, index: usize) -> bool {
        self.window_visible && self.active_tab == Some(index)
    }

    fn is_sleep_candidate(&self, index: usize) -> bool {
        !self.window_visible || self.active_tab != Some(index)
    }

    fn is_pressure_candidate(&self, index: usize) -> bool {
        self.active_tab != Some(index) || !self.window_visible
    }
}
