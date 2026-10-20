//! Notices over the page: crash, restart and zoom chips, and JavaScript dialogs.

use gpui::{
    AnyElement, App, Context, ElementId, Global, SharedString, Task, Window, div, prelude::*, px,
};
use photon_core::BrowserCommand;
use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
};

use super::super::super::engine::ZoomStep;
use super::super::motion::{Speed, Transition, toast};
use super::super::settings::{ConfigReloadNotice, Settings};
use super::super::status_chip::{CHIP_MOTION, ChipIcon, status_chip};
use super::super::theme::ThemeColors;
use super::super::{ClickHandler, CrashNotice, metrics};
use super::BrowserWindow;
use super::BrowserWindowRegistry;
use crate::platform::app_update::update_app;

/// How long a general-purpose notice stays visible.
const ALERT_DURATION: Duration = Duration::from_secs(3);
/// Maximum number of general-purpose alerts shown in the corner stack.
const ALERT_STACK_LIMIT: usize = 3;

/// What the window says about an Engine service that stopped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum EngineNotice {
    Restarting,
    /// The service was running again at this time.
    Restarted(Instant),
}

/// How long a chip announcing a recovery stays up.
const RECOVERED_NOTICE_DURATION: Duration = Duration::from_secs(3);
/// How long the zoom chip stays up after the last zoom change.
const ZOOM_NOTICE_DURATION: Duration = Duration::from_secs(2);

/// One transient notice requested by a shell feature.
#[derive(Clone)]
pub(super) struct AppAlert {
    id: u64,
    message: SharedString,
    expires_at: Instant,
}

/// The active general-purpose notices for each browser window.
#[derive(Default)]
pub(super) struct AlertCenter {
    next_id: u64,
    alerts: HashMap<u64, VecDeque<AppAlert>>,
}

impl Global for AlertCenter {}

impl AlertCenter {
    fn show(&mut self, window_id: u64, message: SharedString) {
        let now = Instant::now();
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let alerts = self.alerts.entry(window_id).or_default();
        alerts.retain(|alert| alert.expires_at > now);
        alerts.push_back(AppAlert {
            id: self.next_id,
            message,
            expires_at: now + ALERT_DURATION,
        });
        while alerts.len() > ALERT_STACK_LIMIT {
            alerts.pop_front();
        }
    }

    pub(super) fn current(window_id: u64, cx: &App) -> Vec<AppAlert> {
        let now = Instant::now();
        cx.try_global::<Self>()
            .and_then(|center| center.alerts.get(&window_id))
            .into_iter()
            .flatten()
            .filter(|alert| alert.expires_at > now)
            .cloned()
            .collect()
    }

    pub(super) fn dismiss_latest(window_id: u64, cx: &mut App) -> bool {
        if !cx
            .try_global::<Self>()
            .and_then(|center| center.alerts.get(&window_id))
            .is_some_and(|alerts| !alerts.is_empty())
        {
            return false;
        }
        cx.update_global::<Self, _>(|center, _| {
            if let Some(alerts) = center.alerts.get_mut(&window_id) {
                alerts.pop_back();
            }
            center.alerts.retain(|_, alerts| !alerts.is_empty());
        });
        true
    }

    pub(super) fn dismiss_alert(window_id: u64, alert_id: u64, cx: &mut App) -> bool {
        let exists = cx
            .try_global::<Self>()
            .and_then(|center| center.alerts.get(&window_id))
            .is_some_and(|alerts| alerts.iter().any(|alert| alert.id == alert_id));
        if !exists {
            return false;
        }
        cx.update_global::<Self, _>(|center, _| {
            if let Some(alerts) = center.alerts.get_mut(&window_id) {
                alerts.retain(|alert| alert.id != alert_id);
            }
            center.alerts.retain(|_, alerts| !alerts.is_empty());
        });
        true
    }

    fn expire(window_id: u64, cx: &mut App) {
        let now = Instant::now();
        if cx.has_global::<Self>() {
            cx.update_global::<Self, _>(|center, _| {
                if let Some(alerts) = center.alerts.get_mut(&window_id) {
                    alerts.retain(|alert| alert.expires_at > now);
                }
                center.alerts.retain(|_, alerts| !alerts.is_empty());
            });
        }
    }

    pub(super) fn remove_window(window_id: u64, cx: &mut App) {
        if cx.has_global::<Self>() {
            cx.update_global::<Self, _>(|center, _| {
                center.alerts.remove(&window_id);
            });
        }
    }
}

/// Shows a short-lived notice in the frontmost Photon browser window.
///
/// The same call works from a window `Context` or from `&mut App`:
/// `show_alert(cx, "Settings saved")`.
/// Notices stack at the bottom right of the active browser window. Each expires
/// independently; the stack shows the three newest notices. Returns `false` when
/// no Photon browser window is open.
pub fn show_alert(cx: &mut App, message: impl Into<SharedString>) -> bool {
    let target = cx
        .active_window()
        .and_then(|window| window.downcast::<BrowserWindow>())
        .or_else(|| {
            if !cx.has_global::<BrowserWindowRegistry>() {
                return None;
            }
            BrowserWindowRegistry::ordered_handles(cx)
                .into_iter()
                .find_map(|window| window.downcast::<BrowserWindow>())
        });
    let Some(window) = target else {
        return false;
    };

    cx.default_global::<AlertCenter>()
        .show(window.window_id().as_u64(), message.into());
    true
}

/// What a chip's button does.
#[derive(Clone, Copy)]
enum NoticeAction {
    /// Reload the page; the chip can also be dismissed.
    Reload,
    /// Stop and restart the WebContent process holding the page's pending input.
    RestartPage,
    /// Return the page to 100%.
    ResetZoom,
}

/// A chip's content.
#[derive(Clone)]
pub(super) struct Notice {
    id: ElementId,
    icon: ChipIcon,
    message: SharedString,
    /// When the chip goes away by itself, if it does.
    expires_at: Option<Instant>,
    action: Option<NoticeAction>,
}

impl BrowserWindow {
    /// Reports a completed, user-visible change in this browser window.
    pub(super) fn show_action_alert(
        &self,
        message: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        cx.default_global::<AlertCenter>()
            .show(self.window_id, message.into());
    }

    /// Shows the JavaScript dialog of the page in view: the preview's while
    /// it has one open, otherwise the active tab's. Focus returns to the
    /// page when the dialog closes.
    pub(super) fn sync_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let active = self.active_webview();
        let peek = self.peek_webview();
        let webview = peek
            .filter(|peek| peek.read(cx).dialogs.open().is_some())
            .or(active);
        if self.dialog.sync(webview.as_ref(), window, cx) {
            cx.notify();
        }
    }

    /// An Engine service stopped (`restarted` false) or is running again.
    pub(super) fn engine_service_changed(&mut self, restarted: bool, cx: &mut Context<Self>) {
        self.engine_notice = Some(if restarted {
            EngineNotice::Restarted(Instant::now())
        } else {
            EngineNotice::Restarting
        });
        self.animate_spinner(cx);
        cx.notify();
    }

    /// Dismisses the shown notice, returning whether there was one.
    pub(super) fn dismiss_notice(&mut self, cx: &mut Context<Self>) -> bool {
        if AlertCenter::dismiss_latest(self.window_id, cx) {
            return true;
        }
        if self.engine_notice.take().is_some() || self.zoom_shown_at.take().is_some() {
            cx.notify();
            return true;
        }
        self.active_webview().is_some_and(|webview| {
            webview.update(cx, |view, cx| {
                let mut dismissed = view.crash_notice.take().is_some();
                if view.state.error.take().is_some() {
                    dismissed = true;
                }
                if dismissed {
                    cx.notify();
                }
                dismissed
            })
        })
    }

    /// Whether the shown notice has a spinner, which needs redrawing as it turns.
    pub(super) fn notice_is_working(&self, cx: &Context<Self>) -> bool {
        self.notice(cx)
            .is_some_and(|notice| matches!(notice.icon, ChipIcon::Working(_)))
    }

    /// The notice to show: an Engine restart first, then a zoom change, then
    /// the active tab's crash or error.
    fn notice(&self, cx: &Context<Self>) -> Option<Notice> {
        let step = self.spinner_step;
        let webview = self.active_webview();
        let zoom_notice = self.zoom_shown_at.and_then(|shown_at| {
            let percent = webview.as_ref()?.read(cx).zoom_percent();
            Some(Notice {
                id: ElementId::Name("zoom".into()),
                icon: ChipIcon::Zoom,
                message: SharedString::from(format!("{percent}%")),
                expires_at: Some(shown_at + ZOOM_NOTICE_DURATION),
                action: (percent != 100).then_some(NoticeAction::ResetZoom),
            })
        });
        let config_notice = Settings::config_reload_notice(cx)
            .map(|notice| match notice {
                ConfigReloadNotice::Reloading => Notice {
                    id: ElementId::Name("config-reloading".into()),
                    icon: ChipIcon::Working(step),
                    message: SharedString::new_static("Reloading config..."),
                    expires_at: None,
                    action: None,
                },
                ConfigReloadNotice::Reloaded(at) => Notice {
                    id: ElementId::Name("config-reloaded".into()),
                    icon: ChipIcon::Done,
                    message: SharedString::new_static("Reloaded!"),
                    expires_at: Some(at + Settings::config_reload_notice_duration()),
                    action: None,
                },
                ConfigReloadNotice::Failed(at) => Notice {
                    id: ElementId::Name("config-reload-failed".into()),
                    icon: ChipIcon::Problem,
                    message: SharedString::new_static("Couldn't reload config"),
                    expires_at: Some(at + Settings::config_reload_notice_duration()),
                    action: None,
                },
            })
            .filter(|notice| {
                !notice
                    .expires_at
                    .is_some_and(|deadline| Instant::now() >= deadline)
            });
        let notice = match self.engine_notice {
            Some(EngineNotice::Restarting) => Notice {
                id: ElementId::Name("engine-restarting".into()),
                icon: ChipIcon::Working(step),
                message: format!("{} stopped. Restarting…", photon_brand::ENGINE_NAME).into(),
                expires_at: None,
                action: None,
            },
            Some(EngineNotice::Restarted(at)) => Notice {
                id: ElementId::Name("engine-restarted".into()),
                icon: ChipIcon::Done,
                message: format!("{} restarted", photon_brand::ENGINE_NAME).into(),
                expires_at: Some(at + RECOVERED_NOTICE_DURATION),
                action: None,
            },
            None if config_notice.is_some() => config_notice?,
            None if webview
                .as_ref()
                .is_some_and(|view| view.read(cx).page_unresponsive) =>
            {
                Notice {
                    id: ElementId::Name("page-unresponsive".into()),
                    icon: ChipIcon::Problem,
                    message: SharedString::new_static("Page isn’t responding"),
                    expires_at: None,
                    action: Some(NoticeAction::RestartPage),
                }
            }
            None if zoom_notice.is_some() => zoom_notice?,
            None => match webview.as_ref()?.read(cx).crash_notice {
                Some(CrashNotice::Reloading) => Notice {
                    id: ElementId::Name("page-reloading".into()),
                    icon: ChipIcon::Working(step),
                    message: SharedString::new_static("This page crashed. Reloading…"),
                    expires_at: None,
                    action: None,
                },
                Some(CrashNotice::Reloaded(at)) => Notice {
                    id: ElementId::Name("page-reloaded".into()),
                    icon: ChipIcon::Done,
                    message: SharedString::new_static("Page reloaded"),
                    expires_at: Some(at + RECOVERED_NOTICE_DURATION),
                    action: None,
                },
                Some(CrashNotice::KeepsCrashing) => Notice {
                    id: ElementId::Name("page-keeps-crashing".into()),
                    icon: ChipIcon::Problem,
                    message: SharedString::new_static("This page keeps crashing"),
                    expires_at: None,
                    action: Some(NoticeAction::Reload),
                },
                None => {
                    // The Engine's error is for the log; the chip says it plainly.
                    webview.as_ref()?.read(cx).state.error.as_ref()?;
                    Notice {
                        id: ElementId::Name("page-error".into()),
                        icon: ChipIcon::Problem,
                        message: SharedString::new_static("This page couldn't be shown"),
                        expires_at: None,
                        action: Some(NoticeAction::Reload),
                    }
                }
            },
        };
        let expired = notice
            .expires_at
            .is_some_and(|deadline| Instant::now() >= deadline);
        (!expired).then_some(notice)
    }

    /// The chip for the current notice, if any.
    pub(super) fn notice_chip(
        &mut self,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let animations = Settings::motion(cx);
        let alerts = AlertCenter::current(self.window_id, cx);
        if !alerts.is_empty() {
            if let Some(deadline) = alerts.iter().map(|alert| alert.expires_at).min() {
                self.expire_notice_at(deadline, cx);
            }

            let count = alerts.len();
            let stack_height = metrics::CHIP_HEIGHT * count as f32
                + metrics::CHIP_GAP * count.saturating_sub(1) as f32;
            let chips = alerts.into_iter().map(|alert| {
                let alert_id = alert.id;
                let dismiss: ClickHandler = Box::new(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    AlertCenter::dismiss_alert(this.window_id, alert_id, cx);
                }));
                status_chip(
                    ElementId::Integer(alert.id),
                    Transition::Enter,
                    toast(),
                    palette,
                    ChipIcon::Info,
                    alert.message,
                    None,
                    Some(dismiss),
                    animations,
                )
                .into_any_element()
            });

            return Some(
                div()
                    .id("alert-stack")
                    .flex()
                    .flex_col()
                    .items_end()
                    .gap(px(metrics::CHIP_GAP))
                    .h(px(stack_height))
                    .transitions(|transitions| {
                        transitions.size(animations.duration(Speed::Standard))
                    })
                    .children(chips)
                    .into_any_element(),
            );
        }

        let notice = self.notice(cx);
        if let Some(deadline) = notice.as_ref().and_then(|notice| notice.expires_at) {
            self.expire_notice_at(deadline, cx);
        }
        let (notice, transition) =
            self.notice_presence
                .sync(notice, CHIP_MOTION, &animations, cx)?;
        let (action, on_dismiss) = match notice.action {
            Some(NoticeAction::Reload) => {
                let reload: ClickHandler = Box::new(cx.listener(|this, _, window, cx| {
                    cx.stop_propagation();
                    this.dispatch_command(BrowserCommand::Reload, window, cx);
                }));
                let dismiss: ClickHandler = Box::new(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.dismiss_notice(cx);
                }));
                (Some(("Reload", reload)), Some(dismiss))
            }
            Some(NoticeAction::ResetZoom) => {
                let reset: ClickHandler = Box::new(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.zoom(ZoomStep::Reset, cx);
                }));
                (Some(("Reset", reset)), None)
            }
            Some(NoticeAction::RestartPage) => {
                let restart: ClickHandler = Box::new(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    if let Some(webview) = this.active_webview() {
                        webview.update(cx, |view, _| view.restart_unresponsive_page());
                    }
                }));
                (Some(("Restart", restart)), None)
            }
            None => (None, None),
        };
        Some(
            status_chip(
                notice.id,
                transition,
                CHIP_MOTION,
                palette,
                notice.icon,
                notice.message,
                action,
                on_dismiss,
                animations,
            )
            .into_any_element(),
        )
    }

    /// Redraws once a recovery notice has had its time, so it disappears.
    fn expire_notice_at(&mut self, deadline: Instant, cx: &mut Context<Self>) {
        if self
            .notice_expiry
            .as_ref()
            .is_some_and(|(scheduled_deadline, _, _)| *scheduled_deadline == deadline)
        {
            return;
        }
        self.notice_expiry_generation = self.notice_expiry_generation.wrapping_add(1);
        let generation = self.notice_expiry_generation;
        let delay = deadline.saturating_duration_since(Instant::now());
        let task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;
            update_app(cx, |cx| {
                this.update(cx, |this, cx| {
                    if this.notice_expiry_generation != generation {
                        return;
                    }
                    this.notice_expiry = None;
                    this.notice_expiry_generation = this.notice_expiry_generation.wrapping_add(1);
                    AlertCenter::expire(this.window_id, cx);
                    if matches!(this.engine_notice, Some(EngineNotice::Restarted(_))) {
                        this.engine_notice = None;
                    }
                    if this
                        .zoom_shown_at
                        .is_some_and(|shown_at| shown_at.elapsed() >= ZOOM_NOTICE_DURATION)
                    {
                        this.zoom_shown_at = None;
                    }
                    cx.notify();
                })
                .ok()
            })
            .await;
        });
        self.notice_expiry = Some((deadline, generation, task));
    }
}

/// Keeps a pending notice expiry alive.
pub(super) type NoticeExpiry = Option<(Instant, u64, Task<()>)>;

#[cfg(test)]
mod tests {
    use super::AlertCenter;

    #[test]
    fn alert_center_keeps_the_three_newest_alerts_per_window() {
        let mut center = AlertCenter::default();
        center.show(1, "first".into());
        center.show(1, "second".into());
        center.show(1, "third".into());
        center.show(1, "fourth".into());
        center.show(2, "another window".into());

        let messages: Vec<_> = center.alerts[&1]
            .iter()
            .map(|alert| alert.message.to_string())
            .collect();
        assert_eq!(messages, ["second", "third", "fourth"]);
        assert_eq!(center.alerts[&2].len(), 1);
    }
}
