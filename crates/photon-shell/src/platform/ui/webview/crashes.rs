//! Recovering a tab whose page process crashed.

use gpui::Context;
use photon_core::{BrowserCommand, CrashResponse};
use std::time::Instant;

use super::super::super::trace;
use super::PhotonWebView;

/// What a tab says about recovering from a crash.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::platform) enum CrashNotice {
    /// The page crashed and is reloading in a fresh process.
    Reloading,
    /// The reloaded page showed at this time.
    Reloaded(Instant),
    /// The page crashed again soon after recovering, so it was not reloaded.
    KeepsCrashing,
}

impl PhotonWebView {
    /// The page's process crashed: reloads it, unless it keeps crashing.
    pub(in crate::platform) fn handle_engine_crash(&mut self, cx: &mut Context<Self>) {
        // The replaced page process no longer waits for its dialog.
        self.dialogs.reset();
        let response = self.crashes.crashed(Instant::now());
        trace(format_args!("page crashed: {response:?}"));
        match response {
            CrashResponse::Reload => {
                self.crash_notice = Some(CrashNotice::Reloading);
                if let Err(error) = self.session.execute(BrowserCommand::Reload) {
                    trace(format_args!("reloading crashed page: {error:#}"));
                }
            }
            CrashResponse::GiveUp => self.crash_notice = Some(CrashNotice::KeepsCrashing),
        }
        self.state_changed(cx);
    }

    /// The page that replaced a crashed one is showing.
    pub(in crate::platform) fn crash_recovered(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        self.crashes.recovered(now);
        trace(format_args!("crashed page recovered"));
        if self.crash_notice == Some(CrashNotice::Reloading) {
            self.crash_notice = Some(CrashNotice::Reloaded(now));
            self.state_changed(cx);
        }
    }
}
