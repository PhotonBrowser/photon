//! Notices over the page: crash, restart and zoom chips, and JavaScript dialogs.

use gpui::{App, Context, ElementId, Global, SharedString, Task, Window, prelude::*};
use photon_core::BrowserCommand;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use super::super::super::engine::ZoomStep;
use super::super::settings::{ConfigReloadNotice, Settings};
use super::super::status_chip::{CHIP_MOTION, ChipIcon, status_chip};
use super::super::theme::ThemeColors;
use super::super::{ClickHandler, CrashNotice};
use super::BrowserWindow;
use super::BrowserWindowRegistry;
use crate::platform::app_update::update_app;

/// How long a general-purpose notice stays visible.
const ALERT_DURATION: Duration = Duration::from_secs(3);

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

/// The active general-purpose notice for each browser window.
#[derive(Default)]
pub(super) struct AlertCenter {
    next_id: u64,
    alerts: HashMap<u64, AppAlert>,
}

impl Global for AlertCenter {}

impl AlertCenter {
    fn show(&mut self, window_id: u64, message: SharedString) {
        self.next_id = self.next_id.wrapping_add(1).max(1);
        self.alerts.insert(
            window_id,
            AppAlert {
                id: self.next_id,
                message,
                expires_at: Instant::now() + ALERT_DURATION,
            },
        );
    }

    pub(super) fn current(window_id: u64, cx: &App) -> Option<&AppAlert> {
        cx.try_global::<Self>()?.alerts.get(&window_id)
    }

    pub(super) fn dismiss(window_id: u64, cx: &mut App) -> bool {
        if !cx
            .try_global::<Self>()
            .is_some_and(|center| center.alerts.contains_key(&window_id))
        {
            return false;
        }
        cx.update_global::<Self, _>(|center, _| {
            center.alerts.remove(&window_id);
        });
        true
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
/// A new notice replaces the previous general-purpose notice in that window.
/// Returns `false` when no Photon browser window is open.
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
        if AlertCenter::dismiss(self.window_id, cx) {
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
        if let Some(alert) = AlertCenter::current(self.window_id, cx)
            && Instant::now() < alert.expires_at
        {
            return Some(Notice {
                id: ElementId::Integer(alert.id),
                icon: ChipIcon::Info,
                message: alert.message.clone(),
                expires_at: Some(alert.expires_at),
                action: None,
            });
        }

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
    ) -> Option<impl IntoElement + use<>> {
        let animations = Settings::motion(cx);
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
        Some(status_chip(
            notice.id,
            transition,
            palette,
            notice.icon,
            notice.message,
            action,
            on_dismiss,
            animations,
        ))
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
