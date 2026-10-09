//! C ABI used by the native shell adapter.

use std::ffi::{CStr, CString, c_char};
use std::ptr;

use photon_core::{BrowserCommand, BrowserState, EngineEvent, normalize_url};

// Keep these values aligned with the public enums in include/photon_ffi.h.
const STRING_URL: u32 = 0;
const STRING_TITLE: u32 = 1;
const STRING_ERROR: u32 = 2;
const FLAG_LOADING: u32 = 0;
const FLAG_CAN_GO_BACK: u32 = 1;
const FLAG_CAN_GO_FORWARD: u32 = 2;
const COMMAND_RELOAD: u32 = 1;
const COMMAND_BACK: u32 = 2;
const COMMAND_FORWARD: u32 = 3;
const COMMAND_STOP_LOADING: u32 = 4;

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
        STRING_URL => handle.url.as_ptr(),
        STRING_TITLE => handle.title.as_ptr(),
        STRING_ERROR => handle.error.as_ptr(),
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
        FLAG_LOADING => handle.navigation_active,
        FLAG_CAN_GO_BACK => handle.state.can_go_back,
        FLAG_CAN_GO_FORWARD => handle.state.can_go_forward,
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
        COMMAND_RELOAD => BrowserCommand::Reload,
        COMMAND_BACK => BrowserCommand::Back,
        COMMAND_FORWARD => BrowserCommand::Forward,
        COMMAND_STOP_LOADING => BrowserCommand::StopLoading,
        _ => return false,
    };
    matches!(
        command,
        BrowserCommand::Reload
            | BrowserCommand::StopLoading
            | BrowserCommand::Back
            | BrowserCommand::Forward
    )
}
