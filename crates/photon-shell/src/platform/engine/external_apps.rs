//! Lets Engine open links it cannot load itself, such as `mailto:` and
//! `tel:`, in other apps through the target OS adapter.

use std::ffi::{CStr, c_char, c_void};

use crate::platform::external_apps;

fn url_from(url: *const c_char) -> Option<String> {
    (!url.is_null()).then(|| {
        unsafe { CStr::from_ptr(url) }
            .to_string_lossy()
            .into_owned()
    })
}

/// Writes the name of the app that opens `url` to `name`, truncated to fit
/// with its terminating NUL, and returns whether there is one.
pub(super) unsafe extern "C" fn on_engine_external_app_name(
    _: *mut c_void,
    url: *const c_char,
    name: *mut c_char,
    capacity: usize,
) -> bool {
    let Some(app) = url_from(url).and_then(|url| external_apps::app_for_url(&url)) else {
        return false;
    };
    if name.is_null() || capacity == 0 {
        return false;
    }
    let mut length = app.len().min(capacity - 1);
    while !app.is_char_boundary(length) {
        length -= 1;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(app.as_ptr().cast::<c_char>(), name, length);
        *name.add(length) = 0;
    }
    true
}

pub(super) unsafe extern "C" fn on_engine_open_external_url(
    _: *mut c_void,
    url: *const c_char,
) -> bool {
    url_from(url).is_some_and(|url| external_apps::open_url(&url))
}
