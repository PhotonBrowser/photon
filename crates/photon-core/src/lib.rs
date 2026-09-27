//! Framework-independent browser state and command decisions for Photon.

use std::ffi::{CStr, CString, c_char};
use std::ptr;
use url::Url;

/// The single active view's persistent browser state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BrowserState {
    pub url: String,
    pub title: String,
    pub loading: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub error: Option<String>,
}

/// Commands submitted by browser chrome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BrowserCommand {
    Navigate(String),
    Reload,
    Back,
    Forward,
}

/// State delivered by the engine adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EngineEvent {
    ViewStateChanged(BrowserState),
    LoadFailed(String),
}

impl BrowserState {
    /// Applies the latest complete state snapshot from LibPhotonEmbedder.
    pub fn apply(&mut self, event: EngineEvent) {
        match event {
            EngineEvent::ViewStateChanged(state) => *self = state,
            EngineEvent::LoadFailed(message) => {
                self.loading = false;
                self.error = Some(message);
            }
        }
    }
}

/// Normalizes an address without guessing a search provider.
pub fn normalize_url(input: &str) -> Result<String, &'static str> {
    let input = input.trim();
    if input.is_empty() {
        return Err("Address is empty");
    }

    let candidate = if input.starts_with("localhost") || looks_like_ip_with_port(input) {
        format!("http://{input}")
    } else if input.starts_with("127.") || input.starts_with("[::1]") {
        format!("http://{input}")
    } else if has_explicit_scheme(input) {
        input.to_owned()
    } else {
        format!("https://{input}")
    };
    let parsed = Url::parse(&candidate).map_err(|_| "Invalid address")?;
    if matches!(parsed.scheme(), "http" | "https" | "ws" | "wss" | "ftp")
        && parsed.host_str().is_none()
    {
        return Err("Address must include a host");
    }
    Ok(parsed.to_string())
}

fn looks_like_ip_with_port(input: &str) -> bool {
    let Some((host, port)) = input.rsplit_once(':') else {
        return false;
    };
    host.parse::<std::net::IpAddr>().is_ok() && port.parse::<u16>().is_ok()
}

fn has_explicit_scheme(input: &str) -> bool {
    let Some((scheme, _)) = input.split_once(':') else {
        return false;
    };
    let mut chars = scheme.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && chars.all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
        })
        && !scheme.contains('.')
}

/// Opaque state allocation used by the C++ Qt adapter.
pub struct BrowserHandle {
    state: BrowserState,
    url: CString,
    title: CString,
    error: CString,
    last_error: CString,
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
    }))
}

/// Destroys state returned by `photon_browser_create`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_destroy(handle: *mut BrowserHandle) {
    if !handle.is_null() {
        drop(unsafe { Box::from_raw(handle) });
    }
}

/// Returns a pointer to a state's null-terminated string field (0 URL, 1 title).
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_flag(handle: *const BrowserHandle, field: u32) -> bool {
    let Some(handle) = (unsafe { handle.as_ref() }) else {
        return false;
    };
    match field {
        0 => handle.state.loading,
        1 => handle.state.can_go_back,
        2 => handle.state.can_go_forward,
        _ => false,
    }
}

/// Applies a full state snapshot from the engine callback.
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
    let state = BrowserState {
        url: read(url),
        title: read(title),
        loading,
        can_go_back: back,
        can_go_forward: forward,
        error: None,
    };
    handle.url = c_string(&state.url);
    handle.title = c_string(&state.title);
    handle.error = CString::default();
    handle.state.apply(EngineEvent::ViewStateChanged(state));
}

/// Maps an address into the engine navigation target and stores any error.
/// Returns 1 on success, 0 on failure. `output` receives a null-terminated URL.
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn photon_browser_error(handle: *const BrowserHandle) -> *const c_char {
    unsafe { handle.as_ref() }.map_or(ptr::null(), |handle| handle.last_error.as_ptr())
}

/// Applies a failure reported by LibPhotonEmbedder.
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
    handle.error = c_string(handle.state.error.as_deref().unwrap_or_default());
}

/// Validates non-navigation commands before the Qt adapter executes them.
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
