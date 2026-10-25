//! System input idle time for avoiding sleep while the user is active.

use std::time::Duration;

const COMBINED_SESSION_STATE: i32 = 0;
const ANY_INPUT_EVENT_TYPE: u32 = u32::MAX;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(state_id: i32, event_type: u32) -> f64;
}

/// Time since the last keyboard, mouse, or tablet event in the login session.
pub(crate) fn duration() -> Option<Duration> {
    // SAFETY: this CoreGraphics function accepts the documented combined-session
    // state and any-input event constants, and reads only the current idle time.
    let seconds = unsafe {
        CGEventSourceSecondsSinceLastEventType(COMBINED_SESSION_STATE, ANY_INPUT_EVENT_TYPE)
    };
    if !seconds.is_finite() || seconds < 0.0 {
        return None;
    }
    Duration::try_from_secs_f64(seconds).ok()
}
