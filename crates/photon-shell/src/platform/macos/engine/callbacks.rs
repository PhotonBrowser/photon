//! The Engine's C callbacks, which update the state in `callback_state`.

use crate::platform::app_update::update_app;
use gpui::CursorStyle;
use mach2::port::mach_port_t;
#[cfg(not(feature = "ui-only"))]
use photon_core::EngineService;
use photon_core::{BrowserState, DialogKind, DialogRequest};
use std::{
    ffi::{CStr, c_char, c_void},
    sync::atomic::{AtomicU64, Ordering},
};

use super::super::ffi::embedder;
use super::super::presentation::MachPortGuard;
pub(super) use super::callback_state::deliver_pending_releases;
pub(super) use super::callback_state::{CallbackState, RuntimeCallbacks};
use super::{EngineSession, RequestedWebView};
use crate::platform::permission_coordinator::PermissionRequest;
use crate::platform::trace;
use crate::platform::ui::{Favicon, FindResult, PageMenuItem, SelectDropdown, SelectItem};
use photon_core::PopupPolicy;

static NEXT_WINDOW_HANDLE: AtomicU64 = AtomicU64::new(1);

/// A string the Engine passed, or `None` for a null pointer.
///
/// # Safety
///
/// `value` is null or a NUL-terminated string valid for this call.
unsafe fn engine_string(value: *const c_char) -> Option<String> {
    if value.is_null() {
        return None;
    }
    Some(
        unsafe { CStr::from_ptr(value) }
            .to_string_lossy()
            .into_owned(),
    )
}

pub(super) unsafe extern "C" fn on_engine_state(
    context: *mut c_void,
    url: *const c_char,
    title: *const c_char,
    loading: bool,
    can_go_back: bool,
    can_go_forward: bool,
    has_connection_security: bool,
    connection_secure: bool,
) {
    let url = unsafe { engine_string(url) };
    let title = unsafe { engine_string(title) }.unwrap_or_default();
    if !loading && let Some(url) = url.as_deref() {
        trace(format_args!("page-state url={url} title={title}"));
    }
    if !context.is_null() {
        let callbacks = unsafe { &*context.cast::<CallbackState>() };
        callbacks.set_page_state(BrowserState {
            url: url.unwrap_or_default(),
            title,
            loading,
            can_go_back,
            can_go_forward,
            connection_secure: has_connection_security.then_some(connection_secure),
            error: None,
        });
    }
}

pub(super) unsafe extern "C" fn on_engine_navigation_timing(
    context: *mut c_void,
    event: i32,
    url: *const c_char,
) {
    if context.is_null() {
        return;
    }
    let Some(url) = (unsafe { engine_string(url) }) else {
        return;
    };
    unsafe { &*(context.cast::<CallbackState>()) }.record_navigation_timing(event, &url);
}
pub(super) unsafe extern "C" fn on_engine_frame(
    context: *mut c_void,
    _: i32,
    _: i32,
    _: usize,
    _: f64,
    _: *const u8,
    _: usize,
    _: u64,
    _: u64,
    buffer_allocation_microseconds: u64,
    copy_microseconds: u64,
    _: u64,
) {
    if !context.is_null() {
        let callbacks = unsafe { &*(context.cast::<CallbackState>()) };
        callbacks.record_cpu_fallback_frame(buffer_allocation_microseconds, copy_microseconds);
        callbacks.request_redraw();
    }
}
pub(super) unsafe extern "C" fn on_engine_cursor(context: *mut c_void, cursor: i32) {
    if context.is_null() {
        return;
    }

    // Keep these discriminants aligned with Photon::Cursor in the embedder API.
    // GPUI-CE has no hidden or wait cursor, so those fall back to Arrow.
    let cursor = match cursor {
        0 | 1 => CursorStyle::Arrow,
        2 => CursorStyle::Crosshair,
        3 => CursorStyle::IBeam,
        4 => CursorStyle::ResizeLeftRight,
        5 => CursorStyle::ResizeUpDown,
        6 => CursorStyle::ResizeUpLeftDownRight,
        7 => CursorStyle::ResizeUpRightDownLeft,
        8 => CursorStyle::ResizeColumn,
        9 => CursorStyle::ResizeRow,
        10 => CursorStyle::PointingHand,
        11 => CursorStyle::ContextualMenu,
        12 => CursorStyle::OpenHand,
        13 | 15 => CursorStyle::ClosedHand,
        14 => CursorStyle::DragCopy,
        17 => CursorStyle::OperationNotAllowed,
        _ => CursorStyle::Arrow,
    };
    unsafe { &*context.cast::<CallbackState>() }
        .update_webview(move |view, cx| view.set_page_cursor(cursor, cx));
}
pub(super) unsafe extern "C" fn on_engine_error(context: *mut c_void, message: *const c_char) {
    if !message.is_null() {
        let message = unsafe { CStr::from_ptr(message) }
            .to_string_lossy()
            .into_owned();
        eprintln!("Photon Engine: {message}");
        if !context.is_null() {
            unsafe { &*(context.cast::<CallbackState>()) }.set_page_error(message);
        }
    }
}

pub(super) unsafe extern "C" fn on_engine_crash(context: *mut c_void, url: *const c_char) {
    let url = unsafe { engine_string(url) }.unwrap_or_else(|| "unknown page".into());
    eprintln!("Photon Engine: WebContent process crashed while displaying {url}");
    if !context.is_null() {
        unsafe { &*(context.cast::<CallbackState>()) }.handle_engine_crash();
    }
}

pub(super) unsafe extern "C" fn on_engine_find_result(
    context: *mut c_void,
    current_match_index: usize,
    has_total: bool,
    total_match_count: usize,
) {
    if context.is_null() {
        return;
    }
    let result = FindResult {
        current_match_index,
        total_match_count: has_total.then_some(total_match_count),
    };
    unsafe { &*(context.cast::<CallbackState>()) }.set_find_result(result);
}

pub(super) unsafe extern "C" fn on_engine_zoom(context: *mut c_void, zoom_level: f64) {
    if !context.is_null() {
        unsafe { &*(context.cast::<CallbackState>()) }.set_zoom_level(zoom_level);
    }
}

pub(super) unsafe extern "C" fn on_engine_page_unresponsive(
    context: *mut c_void,
    unresponsive: bool,
) {
    if !context.is_null() {
        unsafe { &*(context.cast::<CallbackState>()) }.set_page_unresponsive(unresponsive);
    }
}

pub(super) unsafe extern "C" fn on_engine_context_menu(
    context: *mut c_void,
    // The native menu opens at the pointer, where the page was clicked.
    _x: f64,
    _y: f64,
    items: *const embedder::ContextMenuItem,
    count: usize,
) {
    if context.is_null() || (items.is_null() && count > 0) {
        return;
    }
    let items = if count == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(items, count) }
    };
    let items = items
        .iter()
        .map(|item| {
            if item.separator || item.text.is_null() {
                return PageMenuItem::Separator;
            }
            let label = unsafe { CStr::from_ptr(item.text) }.to_string_lossy();
            PageMenuItem::Action {
                label: label.into_owned().into(),
                enabled: item.enabled,
                checked: item.checkable.then_some(item.checked),
            }
        })
        .collect();
    unsafe { &*(context.cast::<CallbackState>()) }
        .update_webview(move |view, cx| view.show_context_menu(items, cx));
}

pub(super) unsafe extern "C" fn on_engine_select_dropdown(
    context: *mut c_void,
    x: f64,
    y: f64,
    minimum_width: f64,
    items: *const embedder::SelectItem,
    count: usize,
) {
    if context.is_null() || (items.is_null() && count > 0) {
        return;
    }
    let items = if count == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(items, count) }
    };
    let items = items
        .iter()
        .map(|item| {
            let label = unsafe { engine_string(item.label) }.unwrap_or_default();
            match item.kind {
                embedder::SELECT_ITEM_OPTION => SelectItem::Option {
                    label: label.into(),
                    id: item.id,
                    selected: item.selected,
                    enabled: item.enabled,
                },
                embedder::SELECT_ITEM_GROUP_LABEL => SelectItem::GroupLabel(label.into()),
                _ => SelectItem::Separator,
            }
        })
        .collect();
    let dropdown = SelectDropdown {
        position: (x as f32, y as f32),
        minimum_width: minimum_width as f32,
        items,
    };
    unsafe { &*(context.cast::<CallbackState>()) }
        .update_webview(move |view, cx| view.show_select_dropdown(dropdown, cx));
}

pub(super) unsafe extern "C" fn on_engine_file_picker(context: *mut c_void, allow_multiple: bool) {
    if !context.is_null() {
        unsafe { &*(context.cast::<CallbackState>()) }
            .update_webview(move |view, cx| view.open_file_picker(allow_multiple, cx));
    }
}

pub(super) unsafe extern "C" fn on_engine_open_in_new_tab(
    context: *mut c_void,
    url: *const c_char,
    activate: bool,
) {
    if context.is_null() || url.is_null() {
        return;
    }
    let url = unsafe { CStr::from_ptr(url) }
        .to_string_lossy()
        .into_owned();
    unsafe { &*(context.cast::<CallbackState>()) }
        .update_webview(move |view, cx| view.open_in_new_tab(url, activate, cx));
}

pub(super) unsafe extern "C" fn on_engine_peek(context: *mut c_void, url: *const c_char) {
    if context.is_null() || url.is_null() {
        return;
    }
    let url = unsafe { CStr::from_ptr(url) }
        .to_string_lossy()
        .into_owned();
    unsafe { &*(context.cast::<CallbackState>()) }
        .update_webview(move |view, cx| view.peek(url, cx));
}

pub(super) unsafe extern "C" fn on_engine_crash_recovered(context: *mut c_void) {
    if !context.is_null() {
        unsafe { &*(context.cast::<CallbackState>()) }.crash_recovered();
    }
}

pub(super) unsafe extern "C" fn on_engine_new_web_view(
    context: *mut c_void,
    _runtime: *mut c_void,
    parent_view: *mut c_void,
    request: *const embedder::NewWebViewRequest,
    window_handle: *mut c_char,
    window_handle_capacity: usize,
) {
    if context.is_null() || request.is_null() {
        return;
    }
    let callbacks = unsafe { &*context.cast::<CallbackState>() };
    let Some(wake) = callbacks.ui_wake.lock().unwrap().clone() else {
        return;
    };
    let request = unsafe { &*request };
    let policy = wake.runtime.popup_policy();
    if request.popup && policy == PopupPolicy::Block {
        trace(format_args!("blocked site popup"));
        return;
    }

    let id = NEXT_WINDOW_HANDLE.fetch_add(1, Ordering::Relaxed);
    let width = request.has_width.then_some(request.width);
    let height = request.has_height.then_some(request.height);
    let viewport_width = width.unwrap_or(1200).max(1);
    let viewport_height = height.unwrap_or(760).max(1);
    let session = match EngineSession::create_for_traversable(
        wake.runtime.clone(),
        parent_view,
        request.traversable,
        viewport_width,
        viewport_height,
        1.0,
    ) {
        Ok(session) => session,
        Err(error) => {
            trace(format_args!("could not create requested page: {error:#}"));
            return;
        }
    };
    session.copy_window_handle(window_handle, window_handle_capacity);
    let pending = RequestedWebView {
        session,
        popup: request.popup,
        activate: request.activate,
        width,
        height,
    };
    wake.app
        .spawn(async move |cx| {
            let Some(webview) = wake.webview.upgrade() else {
                return;
            };
            update_app(cx, |cx| {
                webview.update(cx, |view, cx| view.queue_new_web_view(id, pending, cx));
            })
            .await;
        })
        .detach();
}

/// Called once with the Engine's website data deleted.
pub(super) type BrowsingDataCleared = Box<dyn FnOnce()>;

pub(super) unsafe extern "C" fn on_engine_browsing_data_cleared(context: *mut c_void) {
    if !context.is_null() {
        let done = unsafe { Box::from_raw(context.cast::<BrowsingDataCleared>()) };
        done();
    }
}

/// Called on the main thread with an Engine service's stop or restart.
#[cfg(not(feature = "ui-only"))]
pub(super) type ServiceCallback = Box<dyn Fn(EngineService, bool)>;

#[cfg(not(feature = "ui-only"))]
pub(super) unsafe extern "C" fn on_engine_service(
    context: *mut c_void,
    service: i32,
    restarted: bool,
) {
    if context.is_null() {
        return;
    }
    let service = match service {
        0 => EngineService::Compositor,
        _ => EngineService::Network,
    };
    eprintln!(
        "Photon Engine: {service:?} service {}",
        if restarted { "restarted" } else { "stopped" }
    );
    let on_change = unsafe { &*context.cast::<ServiceCallback>() };
    on_change(service, restarted);
}

pub(super) unsafe extern "C" fn on_engine_performance_stats(
    context: *mut c_void,
    stats: *const embedder::PerformanceStats,
) {
    if context.is_null() || stats.is_null() {
        return;
    }
    unsafe { &*(context.cast::<CallbackState>()) }.set_engine_diagnostics(unsafe { &*stats });
}
pub(super) unsafe extern "C" fn on_engine_favicon(
    context: *mut c_void,
    pixels: *const u8,
    length: usize,
    width: i32,
    height: i32,
) {
    if context.is_null() {
        return;
    }
    let favicon = (!pixels.is_null() && width > 0 && height > 0)
        .then(|| unsafe { std::slice::from_raw_parts(pixels, length) })
        .and_then(|pixels| Favicon::from_bgra(pixels, width as u32, height as u32));
    unsafe { &*(context.cast::<CallbackState>()) }.set_page_favicon(favicon);
}

pub(super) unsafe extern "C" fn on_engine_audio_state(
    context: *mut c_void,
    playing: bool,
    muted: bool,
) {
    if !context.is_null() {
        unsafe { &*(context.cast::<CallbackState>()) }.set_audio_state(playing, muted);
    }
}

pub(super) unsafe extern "C" fn on_engine_permission_request(
    context: *mut c_void,
    request_id: u64,
    document_id: *const c_char,
    requesting_origin: *const c_char,
    top_level_origin: *const c_char,
    capability: *const c_char,
) {
    if context.is_null() {
        return;
    }
    let (Some(document_id), Some(requesting_origin), Some(top_level_origin), Some(capability)) = (
        unsafe { engine_string(document_id) },
        unsafe { engine_string(requesting_origin) },
        unsafe { engine_string(top_level_origin) },
        unsafe { engine_string(capability) },
    ) else {
        return;
    };
    let callbacks = unsafe { &*(context.cast::<CallbackState>()) };
    let request = PermissionRequest {
        view_id: callbacks.task_id,
        engine_request_id: request_id,
        document_id,
        requesting_origin,
        top_level_origin,
        capability,
    };
    callbacks.update_webview(move |view, cx| {
        view.request_permission_prompt(request, cx);
    });
}

pub(super) unsafe extern "C" fn on_engine_permission_cancelled(
    context: *mut c_void,
    request_id: u64,
    document_id: *const c_char,
) {
    if context.is_null() {
        return;
    }
    let Some(document_id) = (unsafe { engine_string(document_id) }) else {
        return;
    };
    unsafe { &*(context.cast::<CallbackState>()) }.update_webview(move |view, cx| {
        view.cancel_permission_prompt(request_id, &document_id, cx);
    });
}

pub(super) unsafe extern "C" fn on_engine_navigation_committed(context: *mut c_void) {
    if !context.is_null() {
        unsafe { &*(context.cast::<CallbackState>()) }.navigation_committed();
    }
}

pub(super) unsafe extern "C" fn on_engine_history_swipe_candidate(
    context: *mut c_void,
    gesture_id: u64,
    back: bool,
    unconsumed_delta: f64,
) {
    if !context.is_null() {
        let callbacks = unsafe { &*(context.cast::<CallbackState>()) };
        callbacks.history_swipe_candidate(gesture_id, back, unconsumed_delta);
    }
}

pub(super) unsafe extern "C" fn on_engine_dialog(
    context: *mut c_void,
    dialog_type: i32,
    title: *const c_char,
    message: *const c_char,
    default_text: *const c_char,
) {
    if context.is_null() {
        return;
    }
    let text = |value: *const c_char| unsafe { engine_string(value) }.unwrap_or_default();
    let kind = match dialog_type {
        0 => DialogKind::Alert,
        1 => DialogKind::Confirm,
        _ => DialogKind::Prompt {
            default_text: text(default_text),
        },
    };
    unsafe { &*(context.cast::<CallbackState>()) }.request_page_dialog(DialogRequest {
        kind,
        title: text(title),
        message: text(message),
    });
}

pub(super) unsafe extern "C" fn on_engine_backing(
    context: *mut c_void,
    backing: u64,
    generation: u64,
    width: u32,
    height: u32,
    format: u32,
    port: u32,
) -> bool {
    if context.is_null() {
        return false;
    }
    let state = unsafe { &*(context.cast::<CallbackState>()) };
    let _owned_port = MachPortGuard(port as mach_port_t);
    let descriptor = photon_presentation_ipc::BackingDescriptor {
        id: backing,
        generation,
        width,
        height,
        pixel_format: format,
    };
    match state
        .presentation
        .register_backing(descriptor, port as mach_port_t)
    {
        Ok(()) => true,
        Err(error) => {
            eprintln!("Photon presentation rejected backing: {error:#}");
            false
        }
    }
}
pub(super) unsafe extern "C" fn on_engine_native_frame(
    context: *mut c_void,
    backing: u64,
    generation: u64,
    frame: u64,
    signal: u64,
    width: i32,
    height: i32,
    _: f64,
) {
    if context.is_null() {
        return;
    }
    let callbacks = unsafe { &*(context.cast::<CallbackState>()) };
    callbacks.record_native_frame();
    callbacks
        .presentation
        .receive_frame(backing, generation, frame, signal, width, height);
    // Receiving a frame queues a release only when it drops one; GPU
    // completion schedules the drain for the rest.
    if callbacks.leases.lock().unwrap().has_pending() {
        callbacks.presentation.schedule_release_drain();
    }
    callbacks.request_redraw();
}

#[cfg(not(feature = "ui-only"))]
pub(super) unsafe extern "C" fn on_native_release_drain(context: *mut c_void) {
    if context.is_null() {
        return;
    }
    let runtime_callbacks = unsafe { &*(context.cast::<RuntimeCallbacks>()) };
    let callbacks = {
        let mut views = runtime_callbacks.views.lock().unwrap();
        let mut callbacks = Vec::with_capacity(views.len());
        views.retain(|view| {
            if let Some(view) = view.upgrade() {
                callbacks.push(view);
                true
            } else {
                false
            }
        });
        callbacks
    };
    for callback in callbacks {
        deliver_pending_releases(&callback);
    }
}
