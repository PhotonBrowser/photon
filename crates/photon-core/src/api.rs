//! C ABI used by the native shell adapter.

use std::ffi::{CStr, CString, c_char};
use std::ptr;

use crate::{BrowserCommand, BrowserState, EngineEvent, normalize_url};

/// Opaque state allocation used by native shell adapters.
pub struct BrowserHandle {
    state: BrowserState,
    url: CString,
    title: CString,
    error: CString,
    last_error: CString,
    committed_url: String,
    navigation_active: bool,
    engine_loading: bool,
    frame_presented: bool,
    restore_url_after_cancel: bool,
}

fn c_string(value: &str) -> CString {
    CString::new(value).unwrap_or_else(|_| CString::default())
}

/// Creates browser state. The caller must eventually call `photon_browser_destroy`.
#[unsafe(no_mangle)]
pub extern "C" fn photon_browser_create() -> *mut BrowserHandle {
    Box::into_raw(Box::new(BrowserHandle {
        state: BrowserState::default(),
        url: CString::default(),
        title: CString::default(),
        error: CString::default(),
        last_error: CString::default(),
        committed_url: String::new(),
        navigation_active: false,
        engine_loading: false,
        frame_presented: true,
        restore_url_after_cancel: false,
    }))
}

/// Destroys state returned by `photon_browser_create`.
///
/// # Safety
/// `handle` must be null or a live pointer returned by `photon_browser_create`, not previously destroyed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_destroy(handle: *mut BrowserHandle) {
    if !handle.is_null() {
        drop(unsafe { Box::from_raw(handle) });
    }
}

/// Returns a pointer to a state's null-terminated string field (0 URL, 1 title).
///
/// # Safety
/// `handle` must be null or point to a live browser handle. The returned pointer is valid only while
/// that handle remains alive and is not mutated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_string(
    handle: *const BrowserHandle,
    field: u32,
) -> *const c_char {
    let Some(handle) = (unsafe { handle.as_ref() }) else {
        return ptr::null();
    };
    match field {
        0 => handle.url.as_ptr(),
        1 => handle.title.as_ptr(),
        2 => handle.error.as_ptr(),
        _ => ptr::null(),
    }
}

/// Returns a boolean state field (0 loading, 1 back, 2 forward).
///
/// # Safety
/// `handle` must be null or point to a live browser handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_flag(handle: *const BrowserHandle, field: u32) -> bool {
    let Some(handle) = (unsafe { handle.as_ref() }) else {
        return false;
    };
    match field {
        0 => handle.navigation_active,
        1 => handle.state.can_go_back,
        2 => handle.state.can_go_forward,
        _ => false,
    }
}

/// Applies a full state snapshot from the engine callback.
///
/// # Safety
/// `handle` must be null or a live mutable browser handle. Non-null `url` and `title` must point to
/// readable null-terminated strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_update(
    handle: *mut BrowserHandle,
    url: *const c_char,
    title: *const c_char,
    loading: bool,
    back: bool,
    forward: bool,
) {
    let Some(handle) = (unsafe { handle.as_mut() }) else {
        return;
    };
    let read = |value: *const c_char| {
        if value.is_null() {
            String::new()
        } else {
            unsafe { CStr::from_ptr(value) }
                .to_string_lossy()
                .into_owned()
        }
    };
    let mut next_url = read(url);
    if handle.restore_url_after_cancel {
        next_url.clone_from(&handle.committed_url);
        if !loading {
            handle.restore_url_after_cancel = false;
        }
    }
    handle.engine_loading = loading;
    if !handle.navigation_active {
        handle.committed_url.clone_from(&next_url);
    }
    if !loading && handle.navigation_active && handle.frame_presented {
        handle.navigation_active = false;
        handle.committed_url.clone_from(&next_url);
    }
    let state = BrowserState {
        url: next_url,
        title: read(title),
        loading: handle.navigation_active,
        can_go_back: back,
        can_go_forward: forward,
        error: None,
    };
    handle.url = c_string(&state.url);
    handle.title = c_string(&state.title);
    handle.error = CString::default();
    handle.state.apply(EngineEvent::ViewStateChanged(state));
}

/// Marks a browser initiated navigation and retains the last displayed address.
///
/// # Safety
/// `handle` must be null or a live mutable browser handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_navigation_started(
    handle: *mut BrowserHandle,
    reuse_displayed_frame: bool,
) {
    let Some(handle) = (unsafe { handle.as_mut() }) else {
        return;
    };
    handle.committed_url.clone_from(&handle.state.url);
    handle.navigation_active = true;
    handle.engine_loading = true;
    handle.frame_presented = reuse_displayed_frame && !handle.state.loading;
    handle.restore_url_after_cancel = false;
    handle.state.loading = true;
}

/// Completes loading after a frame has reached the native presentation scene.
///
/// # Safety
/// `handle` must be null or a live mutable browser handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_frame_presented(handle: *mut BrowserHandle) {
    let Some(handle) = (unsafe { handle.as_mut() }) else {
        return;
    };
    handle.frame_presented = true;
    if handle.navigation_active && !handle.engine_loading {
        handle.navigation_active = false;
        handle.committed_url.clone_from(&handle.state.url);
        handle.state.loading = false;
    }
}

/// Restores the last committed address after stopping an in-flight navigation.
///
/// # Safety
/// `handle` must be null or a live mutable browser handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_cancel_navigation(handle: *mut BrowserHandle) {
    let Some(handle) = (unsafe { handle.as_mut() }) else {
        return;
    };
    handle.navigation_active = false;
    handle.engine_loading = false;
    handle.frame_presented = true;
    handle.restore_url_after_cancel = true;
    handle.state.url.clone_from(&handle.committed_url);
    handle.state.loading = false;
    handle.url = c_string(&handle.state.url);
}

/// Maps an address into the engine navigation target and stores any error.
/// Returns 1 on success, 0 on failure. `output` receives a null-terminated URL.
///
/// # Safety
/// `handle` must be null or a live mutable browser handle. Non-null `input` must be a readable
/// null-terminated string. `output` must be writable for `capacity` bytes when non-null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_navigate(
    handle: *mut BrowserHandle,
    input: *const c_char,
    output: *mut c_char,
    capacity: usize,
) -> bool {
    let Some(handle) = (unsafe { handle.as_mut() }) else {
        return false;
    };
    let input = if input.is_null() {
        ""
    } else {
        unsafe { CStr::from_ptr(input) }.to_str().unwrap_or("")
    };
    match normalize_url(input).map(BrowserCommand::Navigate) {
        Ok(BrowserCommand::Navigate(url)) => {
            let value = c_string(&url);
            let bytes = value.as_bytes_with_nul();
            if output.is_null() || capacity < bytes.len() {
                handle.last_error = c_string("Address is too long");
                return false;
            }
            unsafe { ptr::copy_nonoverlapping(bytes.as_ptr().cast(), output, bytes.len()) };
            handle.last_error = CString::default();
            true
        }
        Err(error) => {
            handle.last_error = c_string(error);
            false
        }
        _ => unreachable!(),
    }
}

/// Returns the last navigation error as a null-terminated string.
///
/// # Safety
/// `handle` must be null or point to a live browser handle. The returned pointer is valid only while
/// that handle remains alive and is not mutated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_error(handle: *const BrowserHandle) -> *const c_char {
    unsafe { handle.as_ref() }.map_or(ptr::null(), |handle| handle.last_error.as_ptr())
}

/// Applies a failure reported by LibPhotonEmbedder.
///
/// # Safety
/// `handle` must be null or a live mutable browser handle. Non-null `message` must point to a readable
/// null-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_load_failed(
    handle: *mut BrowserHandle,
    message: *const c_char,
) {
    let Some(handle) = (unsafe { handle.as_mut() }) else {
        return;
    };
    let message = if message.is_null() {
        "Navigation failed".to_owned()
    } else {
        unsafe { CStr::from_ptr(message) }
            .to_string_lossy()
            .into_owned()
    };
    handle.state.apply(EngineEvent::LoadFailed(message));
    handle.navigation_active = false;
    handle.engine_loading = false;
    handle.frame_presented = true;
    handle.committed_url.clone_from(&handle.state.url);
    handle.error = c_string(handle.state.error.as_deref().unwrap_or_default());
}

/// Validates commands before the native shell adapter executes them.
///
/// # Safety
/// `handle` must be null or point to a live browser handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_command(
    handle: *const BrowserHandle,
    command: u32,
) -> bool {
    if unsafe { handle.as_ref() }.is_none() {
        return false;
    }
    let command = match command {
        1 => BrowserCommand::Reload,
        2 => BrowserCommand::Back,
        3 => BrowserCommand::Forward,
        _ => return false,
    };
    matches!(
        command,
        BrowserCommand::Reload | BrowserCommand::Back | BrowserCommand::Forward
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_domains_and_paths_to_https() {
        assert_eq!(
            normalize_url("example.com").unwrap(),
            "https://example.com/"
        );
        assert_eq!(
            normalize_url("https://example.com/foo").unwrap(),
            "https://example.com/foo"
        );
    }

    #[test]
    fn searches_for_anything_that_is_not_an_address() {
        assert_eq!(
            normalize_url("how to bake bread").unwrap(),
            "https://www.google.com/search?q=how%20to%20bake%20bread"
        );
        assert_eq!(
            normalize_url("rust & golang").unwrap(),
            "https://www.google.com/search?q=rust%20%26%20golang"
        );
    }

    #[test]
    fn normalizes_local_addresses_to_http() {
        assert_eq!(
            normalize_url("localhost:3000").unwrap(),
            "http://localhost:3000/"
        );
        assert_eq!(
            normalize_url("127.0.0.1:8080").unwrap(),
            "http://127.0.0.1:8080/"
        );
        assert_eq!(
            normalize_url("http://localhost:3000").unwrap(),
            "http://localhost:3000/"
        );
    }

    #[test]
    fn preserves_explicit_non_web_schemes() {
        assert_eq!(
            normalize_url("mailto:person@example.com").unwrap(),
            "mailto:person@example.com"
        );
        assert_eq!(normalize_url("about:blank").unwrap(), "about:blank");
    }

    #[test]
    fn trims_input_and_rejects_empty_input() {
        assert_eq!(
            normalize_url("  example.com  ").unwrap(),
            "https://example.com/"
        );
        assert_eq!(normalize_url("  "), Err("Address is empty"));
    }

    #[test]
    fn applies_engine_state_snapshots() {
        let mut state = BrowserState::default();
        state.apply(EngineEvent::ViewStateChanged(BrowserState {
            url: "https://example.com/".into(),
            title: "Example".into(),
            loading: true,
            can_go_back: true,
            can_go_forward: false,
            error: None,
        }));
        assert_eq!(state.url, "https://example.com/");
        assert_eq!(state.title, "Example");
        assert!(state.loading && state.can_go_back);
        assert!(!state.can_go_forward);
    }
}
