//! Display refresh-rate lookup for Engine frame pacing.

use std::ffi::c_void;

/// Engine's own default when a display's rate is unknown.
const FALLBACK_REFRESH_RATE: f64 = 60.0;

#[repr(C)]
struct CVTime {
    time_value: i64,
    time_scale: i32,
    flags: i32,
}

/// CVTime flag set when the time cannot be determined.
const CV_TIME_IS_INDEFINITE: i32 = 1 << 0;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGDisplayCopyDisplayMode(display: u32) -> *mut c_void;
    fn CGDisplayModeGetRefreshRate(mode: *mut c_void) -> f64;
    fn CGDisplayModeRelease(mode: *mut c_void);
}

#[link(name = "CoreVideo", kind = "framework")]
unsafe extern "C" {
    fn CVDisplayLinkCreateWithCGDisplay(display: u32, link: *mut *mut c_void) -> i32;
    fn CVDisplayLinkGetNominalOutputVideoRefreshPeriod(link: *mut c_void) -> CVTime;
    fn CVDisplayLinkRelease(link: *mut c_void);
}

fn usable(rate: f64) -> Option<f64> {
    (rate.is_finite() && rate > 0.0).then_some(rate)
}

fn display_mode_refresh_rate(display: u32) -> Option<f64> {
    let mode = unsafe { CGDisplayCopyDisplayMode(display) };
    if mode.is_null() {
        return None;
    }
    let rate = unsafe { CGDisplayModeGetRefreshRate(mode) };
    unsafe { CGDisplayModeRelease(mode) };
    usable(rate)
}

/// Some panels, including variable-rate ones, report a display mode rate of 0;
/// the display link still knows their nominal refresh period.
fn display_link_refresh_rate(display: u32) -> Option<f64> {
    let mut link = std::ptr::null_mut();
    if unsafe { CVDisplayLinkCreateWithCGDisplay(display, &mut link) } != 0 || link.is_null() {
        return None;
    }
    let period = unsafe { CVDisplayLinkGetNominalOutputVideoRefreshPeriod(link) };
    unsafe { CVDisplayLinkRelease(link) };
    if period.flags & CV_TIME_IS_INDEFINITE != 0 || period.time_value <= 0 {
        return None;
    }
    usable(f64::from(period.time_scale) / period.time_value as f64)
}

/// The refresh rate of a `CGDirectDisplayID`, which is what GPUI's macOS
/// `DisplayId` holds.
pub(super) fn refresh_rate(display: u64) -> f64 {
    let Ok(display) = u32::try_from(display) else {
        return FALLBACK_REFRESH_RATE;
    };
    display_mode_refresh_rate(display)
        .or_else(|| display_link_refresh_rate(display))
        .unwrap_or(FALLBACK_REFRESH_RATE)
}
