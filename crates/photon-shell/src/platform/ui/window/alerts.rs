//! Notices over the page: crash, restart and zoom chips, and JavaScript dialogs.

use gpui::{Context, ElementId, SharedString, Task, Window, prelude::*};
use photon_core::BrowserCommand;
use std::time::{Duration, Instant};

use super::super::super::engine::ZoomStep;
use super::super::js_dialog::JavaScriptDialog;
use super::super::status_chip::{CHIP_MOTION, ChipIcon, status_chip};
use super::super::theme::ThemeColors;
use super::super::{ClickHandler, CrashNotice};
use super::BrowserWindow;

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
    id: &'static str,
    icon: ChipIcon,
    message: SharedString,
    /// When the chip goes away by itself, if it does.
    expires_at: Option<Instant>,
    action: Option<NoticeAction>,
}

impl BrowserWindow {
    /// Shows the active tab's JavaScript dialog, if it has one, and returns
    /// focus to the page when it closes.
    pub(super) fn sync_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(webview) = self.active_webview() else {
            if let Some(dialog) = self.dialog.take() {
                dialog.update(cx, |dialog, cx| dialog.leave(cx));
            }
            return;
        };
        let open = webview.read(cx).dialogs.open().cloned();
        let shown = self
            .dialog
            .as_ref()
            .map(|dialog| dialog.read(cx).request().clone());
        if open == shown {
            return;
        }
        let closed = self.dialog.take();
        let had_focus = closed
            .as_ref()
            .is_some_and(|dialog| dialog.read(cx).contains_focus(window, cx));
        if let Some(closed) = closed {
            closed.update(cx, |dialog, cx| dialog.leave(cx));
        }
        match open {
            Some(request) => {
                let dialog = cx.new(|cx| JavaScriptDialog::new(webview, request, cx));
                dialog.update(cx, |dialog, cx| dialog.focus(window, cx));
                self.dialog = Some(dialog);
            }
            None if had_focus => {
                let page_focus = webview.read(cx).focus_handle.clone();
                window.focus(&page_focus, cx);
            }
            None => {}
        }
        cx.notify();
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
                id: "zoom",
                icon: ChipIcon::Zoom,
                message: SharedString::from(format!("{percent}%")),
                expires_at: Some(shown_at + ZOOM_NOTICE_DURATION),
                action: (percent != 100).then_some(NoticeAction::ResetZoom),
            })
        });
        let notice = match self.engine_notice {
            Some(EngineNotice::Restarting) => Notice {
                id: "engine-restarting",
                icon: ChipIcon::Working(step),
                message: format!("{} stopped. Restarting…", photon_brand::ENGINE_NAME).into(),
                expires_at: None,
                action: None,
            },
            Some(EngineNotice::Restarted(at)) => Notice {
                id: "engine-restarted",
                icon: ChipIcon::Done,
                message: format!("{} restarted", photon_brand::ENGINE_NAME).into(),
                expires_at: Some(at + RECOVERED_NOTICE_DURATION),
                action: None,
            },
            None if webview
                .as_ref()
                .is_some_and(|view| view.read(cx).page_unresponsive) =>
            {
                Notice {
                    id: "page-unresponsive",
                    icon: ChipIcon::Problem,
                    message: SharedString::new_static("Page isn’t responding"),
                    expires_at: None,
                    action: Some(NoticeAction::RestartPage),
                }
            }
            None if zoom_notice.is_some() => zoom_notice?,
            None => match webview.as_ref()?.read(cx).crash_notice {
                Some(CrashNotice::Reloading) => Notice {
                    id: "page-reloading",
                    icon: ChipIcon::Working(step),
                    message: SharedString::new_static("This page crashed. Reloading…"),
                    expires_at: None,
                    action: None,
                },
                Some(CrashNotice::Reloaded(at)) => Notice {
                    id: "page-reloaded",
                    icon: ChipIcon::Done,
                    message: SharedString::new_static("Page reloaded"),
                    expires_at: Some(at + RECOVERED_NOTICE_DURATION),
                    action: None,
                },
                Some(CrashNotice::KeepsCrashing) => Notice {
                    id: "page-keeps-crashing",
                    icon: ChipIcon::Problem,
                    message: SharedString::new_static("This page keeps crashing"),
                    expires_at: None,
                    action: Some(NoticeAction::Reload),
                },
                None => {
                    let error = webview.as_ref()?.read(cx).state.error.clone()?;
                    Notice {
                        id: "page-error",
                        icon: ChipIcon::Problem,
                        message: SharedString::from(format!("Page error: {error}")),
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
        let notice = self.notice(cx);
        if let Some(deadline) = notice.as_ref().and_then(|notice| notice.expires_at) {
            self.expire_notice_at(deadline, cx);
        }
        let (notice, transition) = self.notice_presence.sync(notice, CHIP_MOTION, cx)?;
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
            ElementId::Name(notice.id.into()),
            transition,
            palette,
            notice.icon,
            notice.message,
            action,
            on_dismiss,
        ))
    }

    /// Redraws once a recovery notice has had its time, so it disappears.
    fn expire_notice_at(&mut self, deadline: Instant, cx: &mut Context<Self>) {
        if self.notice_expiry.is_some() {
            return;
        }
        let delay = deadline.saturating_duration_since(Instant::now());
        self.notice_expiry = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;
            this.update(cx, |this, cx| {
                this.notice_expiry = None;
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
            .ok();
        }));
    }
}

/// Keeps a pending notice expiry alive.
pub(super) type NoticeExpiry = Option<Task<()>>;
