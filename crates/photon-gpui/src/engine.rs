//! Photon Engine web surface registered with the GPUIX native renderer.

use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char, c_void};
use std::ptr;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gpui::{Bounds, Corners, LiveImage, Pixels, SharedString};
use gpuix_native::{
    CustomElement, CustomElementFactory, CustomRenderContext, custom_element_surface,
};

mod embedder {
    use super::{c_char, c_void};

    unsafe extern "C" {
        pub fn photon_runtime_create(
            helper_directory: *const c_char,
            error: *mut c_char,
            error_capacity: usize,
        ) -> *mut c_void;
        pub fn photon_runtime_pump(runtime: *mut c_void);
        pub fn photon_runtime_destroy(runtime: *mut c_void);
        pub fn photon_view_create(
            runtime: *mut c_void,
            width: i32,
            height: i32,
            dpr: f64,
            callback_data: *mut c_void,
            state_callback: Option<
                unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char, bool, bool, bool),
            >,
            frame_callback: Option<
                unsafe extern "C" fn(
                    *mut c_void,
                    i32,
                    i32,
                    usize,
                    f64,
                    *const u8,
                    usize,
                    u64,
                    u64,
                    u64,
                    u64,
                ),
            >,
            cursor_callback: Option<unsafe extern "C" fn(*mut c_void, i32)>,
            error_callback: Option<unsafe extern "C" fn(*mut c_void, *const c_char)>,
        ) -> *mut c_void;
        pub fn photon_view_resize(view: *mut c_void, width: i32, height: i32, dpr: f64);
        pub fn photon_view_navigate(view: *mut c_void, url: *const c_char);
        pub fn photon_view_set_focus(view: *mut c_void, focused: bool);
        pub fn photon_view_pointer(
            view: *mut c_void,
            kind: i32,
            x: f64,
            y: f64,
            button: i32,
            buttons: u8,
            shift: bool,
            control: bool,
            alt: bool,
            meta: bool,
            wheel_x: f64,
            wheel_y: f64,
            precise: bool,
            phase: i32,
            clicks: i32,
        );
        pub fn photon_view_key(
            view: *mut c_void,
            key: u16,
            pressed: bool,
            code_point: u32,
            shift: bool,
            control: bool,
            alt: bool,
            meta: bool,
            repeat: bool,
            insert_text: bool,
        );
        pub fn photon_view_shutdown(view: *mut c_void);
        pub fn photon_view_destroy(view: *mut c_void);
    }
}

#[derive(Debug)]
struct PresentedFrame {
    width: i32,
    height: i32,
    dpr: f64,
    pixels: Vec<u8>,
    stride: usize,
    copied_at: Instant,
    engine_paint_interval_us: u64,
    bitmap_acquisition_us: u64,
    native_copy_us: u64,
    rust_copy_us: u64,
    paint_to_callback_us: u64,
}

#[derive(Default)]
struct CallbackState {
    latest_frame: Mutex<Option<PresentedFrame>>,
    reusable_pixels: Mutex<Vec<u8>>,
    latest_cursor: Mutex<Option<i32>>,
    metrics: Mutex<CallbackMetrics>,
}

#[derive(Default)]
struct CallbackMetrics {
    received: u64,
    engine_completed: u64,
    coalesced: u64,
    last_received: Option<Instant>,
    timings_ms: Vec<(&'static str, f64)>,
    vec_allocations: u64,
    bytes_allocated: u64,
    native_frame_allocations: u64,
    native_bytes_allocated: u64,
}

impl CallbackMetrics {
    fn record_received(&mut self, at: Instant, frame: &PresentedFrame) {
        self.received += 1;
        if let Some(previous) = self.last_received {
            self.timings_ms.push((
                "B callback interval",
                at.duration_since(previous).as_secs_f64() * 1000.0,
            ));
        }
        self.last_received = Some(at);
        self.engine_completed += 1;
        self.timings_ms.push((
            "A engine paint interval",
            frame.engine_paint_interval_us as f64 / 1000.0,
        ));
        self.timings_ms.push((
            "A→B Engine complete to callback",
            frame.paint_to_callback_us as f64 / 1000.0,
        ));
        self.timings_ms
            .push(("B→C callback BGRA copy", frame.rust_copy_us as f64 / 1000.0));
        self.timings_ms.push((
            "C bitmap acquisition",
            frame.bitmap_acquisition_us as f64 / 1000.0,
        ));
        self.timings_ms
            .push(("C native BGRA copy", frame.native_copy_us as f64 / 1000.0));
        self.native_frame_allocations += 1;
        self.native_bytes_allocated += frame.pixels.len() as u64;
    }
}

#[derive(Default)]
struct FrameDiagnostics {
    started: Option<Instant>,
    engine_completed: u64,
    received: u64,
    accepted: u64,
    coalesced: u64,
    dropped: u64,
    redraw_requested: u64,
    presented: u64,
    pump_count: u64,
    render_images_created: u64,
    render_images_dropped: u64,
    image_uploads: u64,
    image_recreations: u64,
    vec_allocations: u64,
    bytes_allocated: u64,
    native_frame_allocations: u64,
    native_bytes_allocated: u64,
    surface_vec_allocations: u64,
    surface_bytes_allocated: u64,
    pending_pipeline: Option<[f64; 6]>,
    redraw_requested_at: Option<Instant>,
    timings_ms: Vec<(&'static str, f64)>,
}

impl FrameDiagnostics {
    fn report_if_due(&mut self, width: i32, height: i32, dpr: f64) {
        let now = Instant::now();
        let started = *self.started.get_or_insert(now);
        let elapsed = now.duration_since(started);
        if elapsed < Duration::from_secs(5) {
            return;
        }
        let seconds = elapsed.as_secs_f64();
        eprintln!(
            "Photon profile {width}x{height} DPR {dpr:.2}: FPS Engine={:.1} received={:.1} accepted={:.1} GPUI-presented={:.1}; frames Engine={} received={} accepted={} coalesced={} dropped={} redraws={} presented={} pumps={} RenderImage created={} dropped={} GPUI image uploads={} recreated={} callback Vec alloc={} bytes={} LiveImage Vec alloc={} bytes={} C++ frame Vec alloc={} bytes={} | {}",
            self.engine_completed as f64 / seconds,
            self.received as f64 / seconds,
            self.accepted as f64 / seconds,
            self.presented as f64 / seconds,
            self.engine_completed,
            self.received,
            self.accepted,
            self.coalesced,
            self.dropped,
            self.redraw_requested,
            self.presented,
            self.pump_count,
            self.render_images_created,
            self.render_images_dropped,
            self.image_uploads,
            self.image_recreations,
            self.vec_allocations,
            self.bytes_allocated,
            self.surface_vec_allocations,
            self.surface_bytes_allocated,
            self.native_frame_allocations,
            self.native_bytes_allocated,
            summarize_timings(&mut self.timings_ms)
        );
        *self = Self {
            started: Some(now),
            ..Self::default()
        };
    }
}

fn summarize_timings(timings: &mut Vec<(&'static str, f64)>) -> String {
    timings.sort_by(|a, b| a.0.cmp(b.0));
    let mut output = Vec::new();
    let mut index = 0;
    while index < timings.len() {
        let label = timings[index].0;
        let mut end = index + 1;
        while end < timings.len() && timings[end].0 == label {
            end += 1;
        }
        let values = &mut timings[index..end];
        let count = values.len();
        let average = values.iter().map(|sample| sample.1).sum::<f64>() / count as f64;
        values.sort_by(|a, b| a.1.total_cmp(&b.1));
        let p95 = values[((count as f64 * 0.95).ceil() as usize)
            .saturating_sub(1)
            .min(count - 1)]
        .1;
        let p50 = values[count / 2].1;
        let max = values[count - 1].1;
        output.push(format!(
            "{label}[n={count} avg={average:.2}ms p50={p50:.2}ms p95={p95:.2}ms max={max:.2}ms]"
        ));
        index = end;
    }
    output.join(" ")
}

struct EngineSession {
    runtime: *mut c_void,
    view: *mut c_void,
    callbacks: Box<CallbackState>,
}

impl EngineSession {
    fn create(width: i32, height: i32, dpr: f64) -> Result<Self, String> {
        let helper_directory = std::env::var("PHOTON_HELPER_DIRECTORY")
            .map_err(|_| "PHOTON_HELPER_DIRECTORY is not set".to_string())?;
        let helper_directory = CString::new(helper_directory)
            .map_err(|_| "helper process directory contains a NUL byte".to_string())?;
        let mut error = [0_i8; 1024];
        let runtime = unsafe {
            embedder::photon_runtime_create(
                helper_directory.as_ptr(),
                error.as_mut_ptr(),
                error.len(),
            )
        };
        if runtime.is_null() {
            return Err(unsafe { CStr::from_ptr(error.as_ptr()) }
                .to_string_lossy()
                .into_owned());
        }

        let mut session = Self {
            runtime,
            view: ptr::null_mut(),
            callbacks: Box::default(),
        };
        let view = unsafe {
            embedder::photon_view_create(
                runtime,
                width,
                height,
                dpr,
                (&mut *session.callbacks as *mut CallbackState).cast(),
                Some(on_engine_state),
                Some(on_engine_frame),
                Some(on_engine_cursor),
                Some(on_engine_error),
            )
        };
        if view.is_null() {
            unsafe { embedder::photon_runtime_destroy(runtime) };
            session.runtime = ptr::null_mut();
            return Err("Photon Engine could not create a webpage view".into());
        }
        session.view = view;
        Ok(session)
    }

    fn pump(&mut self) -> Option<PresentedFrame> {
        unsafe { embedder::photon_runtime_pump(self.runtime) };
        self.callbacks.latest_frame.lock().unwrap().take()
    }

    fn recycle_pixels(&self, pixels: Vec<u8>) {
        let mut reusable = self.callbacks.reusable_pixels.lock().unwrap();
        if pixels.capacity() > reusable.capacity() {
            *reusable = pixels;
        }
    }

    fn take_metrics(&self) -> CallbackMetrics {
        std::mem::take(&mut *self.callbacks.metrics.lock().unwrap())
    }

    fn take_cursor(&self) -> Option<i32> {
        self.callbacks.latest_cursor.lock().unwrap().take()
    }

    fn resize(&mut self, width: i32, height: i32, dpr: f64) {
        unsafe { embedder::photon_view_resize(self.view, width, height, dpr) }
    }

    fn navigate(&mut self, url: &str) -> Result<(), String> {
        let url = CString::new(url).map_err(|_| "URL contains a NUL byte".to_string())?;
        unsafe { embedder::photon_view_navigate(self.view, url.as_ptr()) };
        Ok(())
    }
}

unsafe extern "C" fn on_engine_cursor(context: *mut c_void, cursor: i32) {
    if context.is_null() {
        return;
    }
    let callbacks = unsafe { &*(context.cast::<CallbackState>()) };
    *callbacks.latest_cursor.lock().unwrap() = Some(cursor);
}

impl Drop for EngineSession {
    fn drop(&mut self) {
        unsafe {
            if !self.view.is_null() {
                embedder::photon_view_shutdown(self.view);
                embedder::photon_view_destroy(self.view);
                self.view = ptr::null_mut();
            }
            if !self.runtime.is_null() {
                embedder::photon_runtime_destroy(self.runtime);
                self.runtime = ptr::null_mut();
            }
        }
    }
}

unsafe extern "C" fn on_engine_state(
    _context: *mut c_void,
    url: *const c_char,
    _title: *const c_char,
    loading: bool,
    _can_go_back: bool,
    _can_go_forward: bool,
) {
    if !loading && verbose() && !url.is_null() {
        let url = unsafe { CStr::from_ptr(url) }.to_string_lossy();
        eprintln!("Photon page: {url}");
    }
}

unsafe extern "C" fn on_engine_frame(
    context: *mut c_void,
    width: i32,
    height: i32,
    stride: usize,
    dpr: f64,
    pixels: *const u8,
    length: usize,
    engine_paint_interval_us: u64,
    bitmap_acquisition_us: u64,
    native_copy_us: u64,
    paint_to_callback_us: u64,
) {
    let callback_started = Instant::now();
    if context.is_null() || pixels.is_null() || width <= 0 || height <= 0 {
        return;
    }
    let Some(source_length) = stride.checked_mul(height as usize) else {
        return;
    };
    if stride < width as usize * 4 || length < source_length {
        return;
    }
    let source = unsafe { std::slice::from_raw_parts(pixels, source_length) };
    let copy_started = Instant::now();
    let callbacks = unsafe { &*(context.cast::<CallbackState>()) };
    let mut packed_pixels = std::mem::take(&mut *callbacks.reusable_pixels.lock().unwrap());
    if packed_pixels.capacity() < source_length {
        let previous_capacity = packed_pixels.capacity();
        packed_pixels.reserve_exact(source_length - previous_capacity);
        let mut metrics = callbacks.metrics.lock().unwrap();
        metrics.vec_allocations += 1;
        metrics.bytes_allocated += (packed_pixels.capacity() - previous_capacity) as u64;
    }
    packed_pixels.resize(source_length, 0);
    packed_pixels.copy_from_slice(source);
    let rust_copy_us = copy_started.elapsed().as_micros() as u64;
    let copied_at = Instant::now();
    let frame = PresentedFrame {
        width,
        height,
        dpr,
        pixels: packed_pixels,
        stride,
        copied_at,
        engine_paint_interval_us,
        bitmap_acquisition_us,
        native_copy_us,
        rust_copy_us,
        paint_to_callback_us,
    };
    callbacks
        .metrics
        .lock()
        .unwrap()
        .record_received(callback_started, &frame);
    let mut latest_frame = callbacks.latest_frame.lock().unwrap();
    if let Some(replaced) = latest_frame.replace(frame) {
        callbacks.metrics.lock().unwrap().coalesced += 1;
        *callbacks.reusable_pixels.lock().unwrap() = replaced.pixels;
    }
}

unsafe extern "C" fn on_engine_error(context: *mut c_void, message: *const c_char) {
    if context.is_null() || message.is_null() {
        return;
    }
    let message = unsafe { CStr::from_ptr(message) }.to_string_lossy();
    eprintln!("Photon Engine: {message}");
}

#[derive(Default)]
struct WebViewState {
    url: String,
    navigated_url: Option<String>,
    session: Option<EngineSession>,
    image: Option<Arc<LiveImage>>,
    cursor: gpui::CursorStyle,
    // Latest GPUI layout viewport, used to defer initial navigation until the
    // engine has received a usable size.
    viewport: Option<(i32, i32, u32)>,
    bounds_origin: Option<(f32, f32)>,
    focus_subscription: Option<gpui::Subscription>,
    blur_subscription: Option<gpui::Subscription>,
    frame_dimensions: Option<(i32, i32)>,
    rejected_frame_dimensions: Option<(i32, i32)>,
    diagnostics: FrameDiagnostics,
    creation_failed: bool,
}

pub struct PhotonWebViewElement {
    state: Rc<RefCell<WebViewState>>,
}

pub struct PhotonWebViewFactory;

impl CustomElementFactory for PhotonWebViewFactory {
    fn element_type(&self) -> &str {
        "photon-webview"
    }

    fn create(&self, _id: u64) -> Box<dyn CustomElement> {
        Box::new(PhotonWebViewElement {
            state: Rc::new(RefCell::new(WebViewState::default())),
        })
    }
}

gpuix_native::register_custom_element!(|| Box::new(PhotonWebViewFactory));

/// Kept reachable from the addon composition root so the linker includes the
/// inventory registration in the final N-API binary.
pub fn ensure_linked() {
    super::titlebar::ensure_linked();
}

impl CustomElement for PhotonWebViewElement {
    fn render(
        &mut self,
        context: CustomRenderContext,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<gpuix_native::GpuixView>,
    ) -> gpui::AnyElement {
        use gpui::prelude::*;

        let shared = self.state.clone();
        let focus_handle = context
            .focus_handle
            .cloned()
            .unwrap_or_else(|| cx.focus_handle());
        if self.state.borrow().focus_subscription.is_none()
            || self.state.borrow().blur_subscription.is_none()
        {
            let blur_state = shared.clone();
            let focus_state = shared.clone();
            let blur = cx.on_blur(&focus_handle, window, move |_, _, _| {
                set_page_focus(&blur_state, false)
            });
            let focus = cx.on_focus(&focus_handle, window, move |_, _, _| {
                set_page_focus(&focus_state, true)
            });
            let mut state = self.state.borrow_mut();
            state.focus_subscription = Some(focus);
            state.blur_subscription = Some(blur);
        }
        let state = self.state.borrow();
        let image = state.image.clone();
        let cursor = state.cursor;
        let root_id = SharedString::from(format!("photon-webview-{}", context.id()));
        drop(state);
        let timing_state = shared.clone();
        window.on_frame_timing(move |timing| {
            if !verbose() {
                return;
            }
            let mut state = timing_state.borrow_mut();
            let diagnostics = &mut state.diagnostics;
            if timing.image_uploads > 0 {
                diagnostics.presented += 1;
            }
            diagnostics.image_uploads += timing.image_uploads;
            diagnostics.image_recreations += timing.image_recreations;
            diagnostics
                .timings_ms
                .push(("F GPUI scene draw", timing.draw.as_secs_f64() * 1000.0));
            diagnostics.timings_ms.push((
                "F GPUI image atlas upload",
                timing.image_atlas.as_secs_f64() * 1000.0,
            ));
            diagnostics.timings_ms.push((
                "F GPUI platform present",
                timing.present.as_secs_f64() * 1000.0,
            ));
            if timing.image_uploads > 0
                && let Some(mut pipeline) = diagnostics.pending_pipeline.take()
            {
                pipeline[5] = diagnostics
                    .redraw_requested_at
                    .take()
                    .map_or(0.0, |at| at.elapsed().as_secs_f64() * 1000.0);
                diagnostics
                    .timings_ms
                    .push(("E→F redraw-to-present", pipeline[5]));
                diagnostics
                    .timings_ms
                    .push(("A→F end-to-end", pipeline.iter().sum()));
            }
        });

        let mouse_state = shared.clone();
        let down_state = shared.clone();
        let up_state = shared.clone();
        let wheel_state = shared.clone();
        let exit_state = shared.clone();
        let click_focus_handle = focus_handle.clone();
        let keydown_state = shared.clone();
        let keyup_state = shared.clone();
        let root = gpui::div()
            .on_painted(move |bounds, window, _| {
                // Create the engine view from the web surface's own painted
                // bounds. Waiting for a child prepaint callback can leave the
                // initial viewport unavailable until the first interaction.
                apply_viewport(&shared, bounds, window.scale_factor());
                let keydown_state = keydown_state.clone();
                let keyup_state = keyup_state.clone();
                window.on_root_key_event(move |event: &gpui::KeyDownEvent, phase, _, _| {
                    if phase == gpui::DispatchPhase::Capture {
                        send_key(
                            &keydown_state,
                            &event.keystroke,
                            true,
                            event.is_held,
                            key_should_insert_text(&event.keystroke),
                        );
                    }
                });
                window.on_root_key_event(move |event: &gpui::KeyUpEvent, phase, _, _| {
                    if phase == gpui::DispatchPhase::Capture {
                        send_key(&keyup_state, &event.keystroke, false, false, false);
                    }
                });
            })
            .track_focus(&focus_handle)
            .cursor(cursor)
            .on_mouse_move(move |event, _, _| {
                send_pointer(
                    &mouse_state,
                    0,
                    event.position,
                    0,
                    event.pressed_button,
                    event.modifiers,
                    0.0,
                    0.0,
                    false,
                    0,
                    0,
                );
            })
            .on_mouse_exit(move |_, window, _| {
                send_pointer(
                    &exit_state,
                    1,
                    window.mouse_position(),
                    0,
                    None,
                    Default::default(),
                    0.0,
                    0.0,
                    false,
                    0,
                    0,
                );
            })
            .on_any_mouse_down(move |event, window, cx| {
                let button = mouse_button_code(event.button);
                send_pointer(
                    &down_state,
                    2,
                    event.position,
                    button,
                    Some(event.button),
                    event.modifiers,
                    0.0,
                    0.0,
                    false,
                    0,
                    event.click_count as i32,
                );
                window.focus(&click_focus_handle, cx);
                set_page_focus(&down_state, true);
            })
            .capture_any_mouse_up(move |event, _, _| {
                send_pointer(
                    &up_state,
                    3,
                    event.position,
                    mouse_button_code(event.button),
                    None,
                    event.modifiers,
                    0.0,
                    0.0,
                    false,
                    0,
                    event.click_count as i32,
                );
            })
            .on_scroll_wheel(move |event, _, _| {
                // GPUI uses the opposite wheel sign from Ladybird's native
                // adapters. Preserve native line counts and convert each line
                // using the same 40 px step used by Ladybird's AppKit/Qt paths.
                let converted = event.delta.pixel_delta(gpui::px(40.0));
                let (x, y, precise) = (
                    -f64::from(converted.x),
                    -f64::from(converted.y),
                    event.delta.precise(),
                );
                send_pointer(
                    &wheel_state,
                    4,
                    event.position,
                    0,
                    None,
                    event.modifiers,
                    x,
                    y,
                    precise,
                    match event.touch_phase {
                        gpui::TouchPhase::Started => 1,
                        gpui::TouchPhase::Moved => 2,
                        gpui::TouchPhase::Ended => 3,
                        gpui::TouchPhase::Cancelled => 3,
                    },
                    0,
                );
            })
            .id(root_id)
            .size_full();
        let root = custom_element_surface(root, &context).child(gpui::div().absolute().size_full());
        let root = match image {
            Some(image) => {
                let corners = context.style().map_or_else(Corners::default, |style| {
                    let radius = style.border_radius.unwrap_or_default();
                    Corners {
                        top_left: (style.border_top_left_radius.unwrap_or(radius) as f32).into(),
                        top_right: (style.border_top_right_radius.unwrap_or(radius) as f32).into(),
                        bottom_right: (style.border_bottom_right_radius.unwrap_or(radius) as f32)
                            .into(),
                        bottom_left: (style.border_bottom_left_radius.unwrap_or(radius) as f32)
                            .into(),
                    }
                });
                let frame = gpui::canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        let _ = window.paint_live_image(bounds, corners, image.clone());
                    },
                )
                .absolute()
                .size_full();
                root.child(frame)
            }
            None => root,
        };
        root.into_any_element()
    }

    fn set_prop(&mut self, key: &str, value: serde_json::Value) {
        if key == "url" {
            self.state.borrow_mut().url = value.as_str().unwrap_or_default().to_string();
        }
    }

    fn supported_props(&self) -> &'static [&'static str] {
        &["url"]
    }

    fn supported_events(&self) -> &'static [&'static str] {
        &[]
    }

    fn destroy(&mut self) {
        let mut state = self.state.borrow_mut();
        state.session.take();
        state.image.take();
    }

    fn live_dynamic_image(&self) -> Option<Arc<LiveImage>> {
        self.state.borrow().image.clone()
    }

    fn needs_polling(&self) -> bool {
        self.state.borrow().session.is_some()
    }

    fn poll(&mut self) -> bool {
        let mut state = self.state.borrow_mut();
        let url = state
            .viewport
            .filter(|_| state.navigated_url.as_deref() != Some(state.url.as_str()))
            .filter(|_| !state.url.is_empty())
            .map(|_| state.url.clone());
        if let Some(url) = url {
            let navigation = state.session.as_mut().map(|session| session.navigate(&url));
            match navigation {
                Some(Ok(())) => {
                    if verbose() {
                        eprintln!("Photon page: {url}");
                    }
                    state.navigated_url = Some(url);
                }
                Some(Err(error)) => eprintln!("Photon navigation failed: {error}"),
                None => {}
            }
        }

        let Some(session) = state.session.as_mut() else {
            return false;
        };
        let pump_started = Instant::now();
        let frame = session.pump();
        let pump_ms = pump_started.elapsed().as_secs_f64() * 1000.0;
        let metrics = session.take_metrics();
        let cursor = session.take_cursor().map(cursor_style);
        state.diagnostics.pump_count += 1;
        state
            .diagnostics
            .timings_ms
            .push(("Runtime::pump", pump_ms));
        state.diagnostics.received += metrics.received;
        state.diagnostics.engine_completed += metrics.engine_completed;
        state.diagnostics.coalesced += metrics.coalesced;
        state.diagnostics.vec_allocations += metrics.vec_allocations;
        state.diagnostics.bytes_allocated += metrics.bytes_allocated;
        state.diagnostics.native_frame_allocations += metrics.native_frame_allocations;
        state.diagnostics.native_bytes_allocated += metrics.native_bytes_allocated;
        state.diagnostics.timings_ms.extend(metrics.timings_ms);
        let mut cursor_changed = false;
        if let Some(cursor) = cursor
            && state.cursor != cursor
        {
            state.cursor = cursor;
            cursor_changed = true;
        }
        let Some(frame) = frame else {
            if verbose() {
                let (width, height, dpr) = profile_size(state.viewport, state.frame_dimensions);
                state.diagnostics.report_if_due(width, height, dpr);
            }
            return cursor_changed;
        };
        let (width, height, dpr) = (frame.width, frame.height, frame.dpr);
        let expected_dimensions = state.viewport.map(|(width, height, dpr_bits)| {
            let dpr = f64::from(f32::from_bits(dpr_bits));
            (
                (f64::from(width) * dpr).round() as i32,
                (f64::from(height) * dpr).round() as i32,
            )
        });
        if expected_dimensions != Some((width, height)) {
            let dimensions = (width, height);
            if state.rejected_frame_dimensions != Some(dimensions) && verbose() {
                eprintln!(
                    "Photon Engine frame is {width}x{height}; waiting for viewport {}x{}",
                    expected_dimensions.map_or(0, |(width, _)| width),
                    expected_dimensions.map_or(0, |(_, height)| height)
                );
            }
            state.rejected_frame_dimensions = Some(dimensions);
            if verbose() {
                state.diagnostics.dropped += 1;
            }
            if let Some(session) = state.session.as_ref() {
                session.recycle_pixels(frame.pixels);
            }
            return false;
        }
        state.rejected_frame_dimensions = None;
        let resized = state.frame_dimensions != Some((width, height));
        if resized {
            if verbose() {
                eprintln!("Photon Engine frame dimensions: {width}x{height}");
            }
            state.frame_dimensions = Some((width, height));
        }
        if state.image.is_none() {
            let length = (width as usize)
                .saturating_mul(height as usize)
                .saturating_mul(4);
            let Some(image) = LiveImage::new(width as u32, height as u32).map(Arc::new) else {
                eprintln!("Photon could not allocate live BGRA surface");
                if let Some(session) = state.session.as_ref() {
                    session.recycle_pixels(frame.pixels);
                }
                return false;
            };
            state.image = Some(image);
            if verbose() {
                state.diagnostics.surface_vec_allocations += 1;
                state.diagnostics.surface_bytes_allocated += length as u64;
            }
        }
        let image = state.image.as_ref().unwrap().clone();
        let old_capacity = image.byte_capacity();
        let new_length = (width as usize)
            .saturating_mul(height as usize)
            .saturating_mul(4);
        let copy_to_surface_started = Instant::now();
        let updated = image.update_bgra(width as u32, height as u32, frame.stride, &frame.pixels);
        let update_finished = Instant::now();
        let surface_update_ms = update_finished
            .duration_since(copy_to_surface_started)
            .as_secs_f64()
            * 1000.0;
        if verbose() && image.byte_capacity() > old_capacity {
            state.diagnostics.surface_vec_allocations += 1;
            state.diagnostics.surface_bytes_allocated +=
                (image.byte_capacity() - old_capacity).min(new_length) as u64;
        }
        if !updated {
            eprintln!("Photon frame had invalid strided BGRA dimensions");
            if verbose() {
                state.diagnostics.dropped += 1;
            }
            if let Some(session) = state.session.as_ref() {
                session.recycle_pixels(frame.pixels);
            }
            return false;
        }
        let c_to_d_ms = frame.copied_at.elapsed().as_secs_f64() * 1000.0;
        let d_to_e_ms = Instant::now().duration_since(update_finished).as_secs_f64() * 1000.0;
        let pipeline = [
            frame.paint_to_callback_us as f64 / 1000.0,
            frame.rust_copy_us as f64 / 1000.0,
            c_to_d_ms,
            surface_update_ms,
            d_to_e_ms,
            0.0,
        ];
        if verbose() {
            if state.diagnostics.pending_pipeline.is_some() {
                state.diagnostics.coalesced += 1;
            }
            state.diagnostics.accepted += 1;
            state.diagnostics.redraw_requested += 1;
            state.diagnostics.pending_pipeline = Some(pipeline);
            state.diagnostics.redraw_requested_at = Some(Instant::now());
            for (name, value) in [
                ("C→D frame wait before update", pipeline[2]),
                ("D live surface update", pipeline[3]),
                ("D→E surface-to-redraw", pipeline[4]),
            ] {
                state.diagnostics.timings_ms.push((name, value));
            }
            state
                .diagnostics
                .timings_ms
                .push(("D→E GPUI redraw request", pipeline[4]));
        }
        if let Some(session) = state.session.as_ref() {
            session.recycle_pixels(frame.pixels);
        }
        if verbose() {
            state.diagnostics.report_if_due(width, height, dpr);
        }
        true
    }
}

fn set_page_focus(state: &Rc<RefCell<WebViewState>>, focused: bool) {
    if verbose() {
        eprintln!("PhotonWebView: page focus={focused}");
    }
    if let Some(session) = state.borrow().session.as_ref() {
        unsafe { embedder::photon_view_set_focus(session.view, focused) }
    }
}

fn cursor_style(cursor: i32) -> gpui::CursorStyle {
    use gpui::CursorStyle as Style;
    match cursor {
        1 => Style::Arrow, // Hidden cursors are not exposed by GPUI.
        2 => Style::Crosshair,
        3 => Style::IBeam,
        4 => Style::ResizeLeftRight,
        5 => Style::ResizeUpDown,
        6 => Style::ResizeUpLeftDownRight,
        7 => Style::ResizeUpRightDownLeft,
        8 => Style::ResizeColumn,
        9 => Style::ResizeRow,
        10 => Style::PointingHand,
        11 => Style::ContextualMenu,
        12 => Style::OpenHand,
        13 | 15 => Style::ClosedHand,
        14 => Style::DragCopy,
        16 => Style::Arrow,
        17 => Style::OperationNotAllowed,
        _ => Style::Arrow,
    }
}

fn mouse_button_code(button: gpui::MouseButton) -> i32 {
    match button {
        gpui::MouseButton::Left => 1,
        gpui::MouseButton::Right => 2,
        gpui::MouseButton::Middle => 4,
        gpui::MouseButton::Navigate(gpui::NavigationDirection::Back) => 8,
        gpui::MouseButton::Navigate(gpui::NavigationDirection::Forward) => 16,
    }
}

fn send_pointer(
    state: &Rc<RefCell<WebViewState>>,
    kind: i32,
    position: gpui::Point<Pixels>,
    button: i32,
    pressed: Option<gpui::MouseButton>,
    modifiers: gpui::Modifiers,
    wheel_x: f64,
    wheel_y: f64,
    precise: bool,
    phase: i32,
    clicks: i32,
) {
    let (x, y) = state
        .borrow()
        .bounds_origin
        .map(|(x, y)| {
            (
                f64::from(position.x) - f64::from(x),
                f64::from(position.y) - f64::from(y),
            )
        })
        .unwrap_or_default();
    let buttons = pressed.map_or(0, |button| mouse_button_code(button) as u8);
    send_pointer_at(
        state, kind, x, y, button, buttons, modifiers, wheel_x, wheel_y, precise, phase, clicks,
    );
}

fn send_pointer_at(
    state: &Rc<RefCell<WebViewState>>,
    kind: i32,
    x: f64,
    y: f64,
    button: i32,
    buttons: u8,
    modifiers: gpui::Modifiers,
    wheel_x: f64,
    wheel_y: f64,
    precise: bool,
    phase: i32,
    clicks: i32,
) {
    if let Some(session) = state.borrow().session.as_ref() {
        unsafe {
            embedder::photon_view_pointer(
                session.view,
                kind,
                x,
                y,
                button,
                buttons,
                modifiers.shift,
                modifiers.control,
                modifiers.alt,
                modifiers.platform,
                wheel_x,
                wheel_y,
                precise,
                phase,
                clicks,
            )
        }
    }
}

fn send_key(
    state: &Rc<RefCell<WebViewState>>,
    key: &gpui::Keystroke,
    pressed: bool,
    repeat: bool,
    insert_text: bool,
) {
    let text_character = key_text_character(key);
    let code_point = text_character
        .and_then(|text| text.chars().next())
        .map(u32::from)
        .unwrap_or(0);
    let virtual_key = engine_key_code(&key.key);
    if verbose() && pressed {
        eprintln!(
            "PhotonWebView: key down key={:?} key_char={:?} text={:?} code_point=U+{:04X} insert_text={insert_text} modifiers={:?}",
            key.key, key.key_char, text_character, code_point, key.modifiers
        );
    }
    if let Some(session) = state.borrow().session.as_ref() {
        unsafe {
            embedder::photon_view_key(
                session.view,
                virtual_key,
                pressed,
                code_point,
                key.modifiers.shift,
                key.modifiers.control,
                key.modifiers.alt,
                key.modifiers.platform,
                repeat,
                insert_text,
            )
        }
    }
}

fn key_should_insert_text(key: &gpui::Keystroke) -> bool {
    text_key_is_unmodified(key) && key_text_character(key).is_some()
}

fn text_key_is_unmodified(key: &gpui::Keystroke) -> bool {
    (!key.modifiers.control && !key.modifiers.platform) || key.modifiers.alt
}

fn key_text_character(key: &gpui::Keystroke) -> Option<&str> {
    if !text_key_is_unmodified(key) {
        return None;
    }

    if let Some(text) = key.key_char.as_deref() {
        return Some(text);
    }

    // GPUI normally supplies key_char, but some platform/IME paths only
    // provide the printable key name to custom elements without an InputHandler.
    if key.key == "space" {
        return Some(" ");
    }

    let mut characters = key.key.chars();
    let character = characters.next()?;
    (characters.next().is_none() && !character.is_control()).then_some(&key.key)
}

fn engine_key_code(key: &str) -> u16 {
    match key {
        "backspace" => 0x08,
        "tab" => 0x09,
        "enter" => 0x0d,
        "escape" => 0x1b,
        "space" => 0x20,
        "pageup" => 0x21,
        "pagedown" => 0x22,
        "end" => 0x23,
        "home" => 0x24,
        "left" => 0x25,
        "up" => 0x26,
        "right" => 0x27,
        "down" => 0x28,
        "delete" => 0x2e,
        "f1" | "f2" | "f3" | "f4" | "f5" | "f6" | "f7" | "f8" | "f9" | "f10" | "f11" | "f12" => {
            0x70 + key[1..].parse::<u16>().unwrap_or(1) - 1
        }
        _ if key.len() == 1 => key.as_bytes()[0].to_ascii_uppercase() as u16,
        _ => 0,
    }
}

fn apply_viewport(state: &Rc<RefCell<WebViewState>>, bounds: Bounds<Pixels>, scale: f32) {
    let width = f32::from(bounds.size.width).max(0.0);
    let height = f32::from(bounds.size.height).max(0.0);
    let dpr = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    if width < 1.0 || height < 1.0 {
        return;
    }

    let logical_width = width.round() as i32;
    let logical_height = height.round() as i32;
    let physical_width = (logical_width as f32 * dpr).round() as i32;
    let physical_height = (logical_height as f32 * dpr).round() as i32;
    let viewport = (logical_width, logical_height, dpr.to_bits());
    let should_create = {
        let mut state = state.borrow_mut();
        state.bounds_origin = Some((f32::from(bounds.origin.x), f32::from(bounds.origin.y)));
        if state.viewport != Some(viewport) {
            state.viewport = Some(viewport);
            if let Some(session) = state.session.as_mut() {
                session.resize(logical_width, logical_height, f64::from(dpr));
            }
            if verbose() {
                eprintln!(
                    "PhotonWebView: logical={}x{} physical={}x{} dpr={dpr}",
                    logical_width, logical_height, physical_width, physical_height
                );
            }
        }
        state.session.is_none() && !state.creation_failed
    };

    if should_create {
        match EngineSession::create(logical_width, logical_height, f64::from(dpr)) {
            Ok(session) => state.borrow_mut().session = Some(session),
            Err(error) => {
                eprintln!("Photon Engine startup failed: {error}");
                state.borrow_mut().creation_failed = true;
            }
        }
    }
}

fn verbose() -> bool {
    std::env::var_os("PHOTON_VERBOSE").is_some()
}

fn profile_size(viewport: Option<(i32, i32, u32)>, frame: Option<(i32, i32)>) -> (i32, i32, f64) {
    if let Some((width, height, dpr_bits)) = viewport {
        return (width, height, f64::from(f32::from_bits(dpr_bits)));
    }
    frame.map_or((0, 0, 1.0), |(width, height)| (width, height, 1.0))
}
