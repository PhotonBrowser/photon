//! Routes SIGINT and SIGTERM through GPUI so app-quit hooks can shut down the
//! Engine and its helper processes in order.

use gpui::App;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const POLL_INTERVAL: Duration = Duration::from_millis(50);

static REQUESTED: AtomicBool = AtomicBool::new(false);

extern "C" fn record_request(_: libc::c_int) {
    REQUESTED.store(true, Ordering::Relaxed);
}

/// Replaces the default signal handlers, which would end the process at once.
pub(crate) fn install() {
    REQUESTED.store(false, Ordering::Relaxed);
    // SAFETY: the handler only stores to an atomic, which is signal-safe.
    unsafe {
        libc::signal(
            libc::SIGINT,
            record_request as *const () as libc::sighandler_t,
        );
        libc::signal(
            libc::SIGTERM,
            record_request as *const () as libc::sighandler_t,
        );
    }
}

/// Quits the app once a termination signal has arrived.
pub(crate) fn quit_when_requested(cx: &mut App) {
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(POLL_INTERVAL).await;
            if REQUESTED.load(Ordering::Relaxed) {
                cx.update(|cx| cx.quit());
                break;
            }
        }
    })
    .detach();
}
