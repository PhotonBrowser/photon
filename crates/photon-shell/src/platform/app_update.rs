//! Updating the app from tasks that native callbacks start.
//!
//! AppKit and the Engine sometimes spin a nested event loop while the app is
//! being updated, as when the menu bar is replaced while the Dock restarts.
//! A task queued from a native callback can run inside that loop, where the
//! app cannot be updated; it then waits a moment and tries again.

use gpui::{App, AsyncApp};
use std::time::Duration;

/// How long a task waits before trying the app again.
const RETRY_DELAY: Duration = Duration::from_millis(8);
/// How many times it tries before giving up, as when the app has gone.
const ATTEMPTS: u32 = 500;

/// Runs `update` with the app as soon as it is free. Returns `None` when it
/// never was, as when the app quit meanwhile.
pub(crate) async fn update_app<R>(cx: &AsyncApp, update: impl FnOnce(&mut App) -> R) -> Option<R> {
    let mut update = Some(update);
    for _ in 0..ATTEMPTS {
        if let Some(result) = cx.try_update(|cx| (update.take().expect("runs once"))(cx)) {
            return Some(result);
        }
        cx.background_executor().timer(RETRY_DELAY).await;
    }
    None
}
