use std::{ffi::c_void, os::raw::c_int};

use crate::{WindowObservation, WindowObservationError};

use super::process::{process_matches_session_leader, process_session_id, session_is_alive};

type CFTypeRef = *const c_void;
type CFStringRef = CFTypeRef;
type CFArrayRef = CFTypeRef;
type CFDictionaryRef = CFTypeRef;

const WINDOW_LIST_ON_SCREEN_ONLY: u32 = 1 << 0;
const WINDOW_LIST_EXCLUDE_DESKTOP_ELEMENTS: u32 = 1 << 4;
const CF_NUMBER_INT_TYPE: isize = 9;
const CF_NUMBER_DOUBLE_TYPE: isize = 13;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGWindowListCopyWindowInfo(option: u32, relative_to_window: u32) -> CFArrayRef;

    static kCGWindowNumber: CFStringRef;
    static kCGWindowOwnerPID: CFStringRef;
    static kCGWindowLayer: CFStringRef;
    static kCGWindowAlpha: CFStringRef;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFArrayGetCount(array: CFArrayRef) -> isize;
    fn CFArrayGetValueAtIndex(array: CFArrayRef, index: isize) -> CFTypeRef;
    fn CFDictionaryGetValue(dictionary: CFDictionaryRef, key: CFTypeRef) -> CFTypeRef;
    fn CFNumberGetValue(number: CFTypeRef, number_type: isize, value: *mut c_void) -> u8;
    fn CFRelease(value: CFTypeRef);
}

/// Observes on-screen application windows through Quartz Window Services.
/// Window ownership is accepted only when the owner PID still belongs to the
/// private process session created for the current Velocity launch.
#[derive(Clone, Copy, Debug, Default)]
pub struct MacOSWindowObserver;

impl crate::BrowserWindowObserver for MacOSWindowObserver {
    fn observe(
        &self,
        process_id: u32,
        managed_session_id: u32,
        executable_path: &std::path::Path,
    ) -> Result<WindowObservation, WindowObservationError> {
        let verified_leader =
            process_matches_session_leader(process_id, managed_session_id, executable_path)
                .map_err(|error| WindowObservationError(error.to_string()))?;
        if !verified_leader {
            return if session_is_alive(managed_session_id)
                .map_err(|error| WindowObservationError(error.to_string()))?
            {
                Ok(WindowObservation::OwnershipUncertain)
            } else {
                Ok(WindowObservation::ProcessExited)
            };
        }

        let options = WINDOW_LIST_ON_SCREEN_ONLY | WINDOW_LIST_EXCLUDE_DESKTOP_ELEMENTS;
        // SAFETY: Quartz returns an owned CFArray of window dictionaries or null.
        let windows = unsafe { CGWindowListCopyWindowInfo(options, 0) };
        if windows.is_null() {
            return Err(WindowObservationError(
                "CoreGraphics could not access the current window-server session".to_owned(),
            ));
        }

        let mut visible = Vec::new();
        // SAFETY: `windows` is a valid CFArray returned above and remains
        // retained until the matching CFRelease after iteration.
        let count = unsafe { CFArrayGetCount(windows) };
        for index in 0..count {
            // SAFETY: index is within the array count and each element is a
            // window-information dictionary as documented by Quartz.
            let dictionary = unsafe { CFArrayGetValueAtIndex(windows, index) };
            if dictionary.is_null() {
                continue;
            }
            let Some(owner_pid) = dictionary_int(dictionary, unsafe { kCGWindowOwnerPID }) else {
                continue;
            };
            if owner_pid <= 0 {
                continue;
            }
            let Some(owner_session_id) = process_session_id(owner_pid as u32) else {
                continue;
            };
            if !window_owner_matches(owner_pid as u32, owner_session_id, managed_session_id) {
                continue;
            }
            let Some(layer) = dictionary_int(dictionary, unsafe { kCGWindowLayer }) else {
                continue;
            };
            let Some(alpha) = dictionary_double(dictionary, unsafe { kCGWindowAlpha }) else {
                continue;
            };
            let Some(window_id) = dictionary_int(dictionary, unsafe { kCGWindowNumber }) else {
                continue;
            };
            if layer == 0 && alpha > 0.0 {
                visible.push(window_id as u32);
            }
        }
        // SAFETY: the array was returned as an owned CoreFoundation object.
        unsafe { CFRelease(windows) };

        visible.sort_unstable();
        visible.dedup();
        Ok(WindowObservation::VisibleWindows(visible))
    }
}

fn dictionary_int(dictionary: CFDictionaryRef, key: CFTypeRef) -> Option<c_int> {
    // SAFETY: all window dictionaries and keys are supplied by CoreGraphics.
    let number = unsafe { CFDictionaryGetValue(dictionary, key) };
    if number.is_null() {
        return None;
    }
    let mut value = 0;
    // SAFETY: CFNumber writes an integer to the correctly sized local value.
    let success = unsafe {
        CFNumberGetValue(
            number,
            CF_NUMBER_INT_TYPE,
            (&mut value as *mut c_int).cast(),
        )
    };
    (success != 0).then_some(value)
}

fn dictionary_double(dictionary: CFDictionaryRef, key: CFTypeRef) -> Option<f64> {
    // SAFETY: all window dictionaries and keys are supplied by CoreGraphics.
    let number = unsafe { CFDictionaryGetValue(dictionary, key) };
    if number.is_null() {
        return None;
    }
    let mut value = 0.0;
    // SAFETY: CFNumber writes a double to the correctly sized local value.
    let success = unsafe {
        CFNumberGetValue(
            number,
            CF_NUMBER_DOUBLE_TYPE,
            (&mut value as *mut f64).cast(),
        )
    };
    (success != 0).then_some(value)
}

fn window_owner_matches(owner_pid: u32, actual_session_id: u32, expected_session_id: u32) -> bool {
    owner_pid > 0 && actual_session_id == expected_session_id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_owner_must_share_the_managed_process_session() {
        assert!(window_owner_matches(42, 7, 7));
        assert!(!window_owner_matches(42, 8, 7));
        assert!(!window_owner_matches(0, 7, 7));
    }
}
