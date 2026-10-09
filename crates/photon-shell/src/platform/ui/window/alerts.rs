//! Notices over the page: crash and restart chips, and JavaScript dialogs.

use gpui::{Context, ElementId, SharedString, Task, Window, prelude::*};
use photon_core::BrowserCommand;
use std::time::{Duration, Instant};

use super::super::js_dialog::JavaScriptDialog;
use super::super::status_chip::{ChipIcon, status_chip};
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

/// A chip's content.
struct Notice {
    id: &'static str,
    icon: ChipIcon,
    message: &'static str,
    /// When a recovery notice was shown, so it can expire.
    recovered_at: Option<Instant>,
    /// The page keeps crashing, so offer to reload it and to dismiss.
    actionable: bool,
}

impl BrowserWindow {
    /// Shows the active tab's JavaScript dialog, if it has one, and returns
    /// focus to the page when it closes.
    pub(super) fn sync_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let webview = self.active_webview();
        let open = webview.read(cx).dialogs.open().cloned();
        let shown = self
            .dialog
            .as_ref()
            .map(|dialog| dialog.read(cx).request().clone());
        if open == shown {
            return;
        }
        let had_focus = self
            .dialog
            .take()
            .is_some_and(|dialog| dialog.read(cx).contains_focus(window, cx));
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
        if self.engine_notice.take().is_some() {
            cx.notify();
            return true;
        }
        self.active_webview().update(cx, |view, cx| {
            let dismissed = view.crash_notice.take().is_some();
            if dismissed {
                cx.notify();
            }
            dismissed
        })
    }

    /// Whether the shown notice has a spinner, which needs redrawing as it turns.
    pub(super) fn notice_is_working(&self, cx: &Context<Self>) -> bool {
        self.notice(cx)
            .is_some_and(|notice| matches!(notice.icon, ChipIcon::Working(_)))
    }

    /// The notice to show: an Engine restart first, then the active tab's crash.
    fn notice(&self, cx: &Context<Self>) -> Option<Notice> {
        let step = self.spinner_step;
        let notice = match self.engine_notice {
            Some(EngineNotice::Restarting) => Notice {
                id: "engine-restarting",
                icon: ChipIcon::Working(step),
                message: "Photon Engine stopped. Restarting…",
                recovered_at: None,
                actionable: false,
            },
            Some(EngineNotice::Restarted(at)) => Notice {
                id: "engine-restarted",
                icon: ChipIcon::Done,
                message: "Photon Engine restarted",
                recovered_at: Some(at),
                actionable: false,
            },
            None => match self.active_webview().read(cx).crash_notice? {
                CrashNotice::Reloading => Notice {
                    id: "page-reloading",
                    icon: ChipIcon::Working(step),
                    message: "This page crashed. Reloading…",
                    recovered_at: None,
                    actionable: false,
                },
                CrashNotice::Reloaded(at) => Notice {
                    id: "page-reloaded",
                    icon: ChipIcon::Done,
                    message: "Page reloaded",
                    recovered_at: Some(at),
                    actionable: false,
                },
                CrashNotice::KeepsCrashing => Notice {
                    id: "page-keeps-crashing",
                    icon: ChipIcon::Problem,
                    message: "This page keeps crashing",
                    recovered_at: None,
                    actionable: true,
                },
            },
        };
        let expired = notice
            .recovered_at
            .is_some_and(|at| at.elapsed() >= RECOVERED_NOTICE_DURATION);
        (!expired).then_some(notice)
    }

    /// The chip for the current notice, if any.
    pub(super) fn notice_chip(
        &mut self,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let notice = self.notice(cx)?;
        if let Some(at) = notice.recovered_at {
            self.expire_notice_at(at + RECOVERED_NOTICE_DURATION, cx);
        }
        let (action, on_dismiss) = if notice.actionable {
            let reload: ClickHandler = Box::new(cx.listener(|this, _, window, cx| {
                cx.stop_propagation();
                this.dispatch_command(BrowserCommand::Reload, window, cx);
            }));
            let dismiss: ClickHandler = Box::new(cx.listener(|this, _, _, cx| {
                cx.stop_propagation();
                this.dismiss_notice(cx);
            }));
            (Some(("Reload", reload)), Some(dismiss))
        } else {
            (None, None)
        };
        Some(status_chip(
            ElementId::Name(notice.id.into()),
            palette,
            notice.icon,
            SharedString::new_static(notice.message),
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
                cx.notify();
            })
            .ok();
        }));
    }
}

/// Keeps a pending notice expiry alive.
pub(super) type NoticeExpiry = Option<Task<()>>;
