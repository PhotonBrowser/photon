//! Updating GPUI after asynchronous callbacks.
//!
//! A task can reach GPUI while a nested native event loop is active, so the
//! update helper waits briefly and retries until the app is available.

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
