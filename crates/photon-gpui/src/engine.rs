//! Photon Engine web surface registered with the GPUIX native renderer.

use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char, c_void};
use std::ptr;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gpui::{Bounds, ObjectFit, Pixels, RenderImage, SharedString};
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
                unsafe extern "C" fn(*mut c_void, i32, i32, usize, f64, *const u8, usize),
            >,
            error_callback: Option<unsafe extern "C" fn(*mut c_void, *const c_char)>,
        ) -> *mut c_void;
        pub fn photon_view_resize(view: *mut c_void, width: i32, height: i32, dpr: f64);
        pub fn photon_view_navigate(view: *mut c_void, url: *const c_char);
        pub fn photon_view_shutdown(view: *mut c_void);
        pub fn photon_view_destroy(view: *mut c_void);
    }
}

#[derive(Debug)]
struct PresentedFrame {
    width: i32,
    height: i32,
    stride: usize,
    dpr: f64,
    pixels: Vec<u8>,
}

#[derive(Default)]
struct CallbackState {
    latest_frame: Mutex<Option<PresentedFrame>>,
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

    fn resize(&mut self, width: i32, height: i32, dpr: f64) {
        unsafe { embedder::photon_view_resize(self.view, width, height, dpr) }
    }

    fn navigate(&mut self, url: &str) -> Result<(), String> {
        let url = CString::new(url).map_err(|_| "URL contains a NUL byte".to_string())?;
        unsafe { embedder::photon_view_navigate(self.view, url.as_ptr()) };
        Ok(())
    }
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
) {
    if context.is_null() || pixels.is_null() || width <= 0 || height <= 0 {
        return;
    }
    let Some(expected) = stride.checked_mul(height as usize) else {
        return;
    };
    if stride < width as usize * 4 || length < expected {
        return;
    }
    let bytes = unsafe { std::slice::from_raw_parts(pixels, expected) }.to_vec();
    let callbacks = unsafe { &*(context.cast::<CallbackState>()) };
    *callbacks.latest_frame.lock().unwrap() = Some(PresentedFrame {
        width,
        height,
        stride,
        dpr,
        pixels: bytes,
    });
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
    image: Option<Arc<RenderImage>>,
    // Latest GPUI layout viewport, used to defer initial navigation until the
    // engine has received a usable size.
    viewport: Option<(i32, i32, u32)>,
    frame_dimensions: Option<(i32, i32)>,
    rejected_frame_dimensions: Option<(i32, i32)>,
    frame_stats_started: Option<Instant>,
    frames_since_report: u64,
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
pub fn ensure_linked() {}

impl CustomElement for PhotonWebViewElement {
    fn render(
        &mut self,
        context: CustomRenderContext,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<gpuix_native::GpuixView>,
    ) -> gpui::AnyElement {
        use gpui::prelude::*;

        let shared = self.state.clone();
        let state = self.state.borrow();
        let image = state.image.clone();
        let root_id = SharedString::from(format!("photon-webview-{}", context.id()));
        drop(state);

        let root = gpui::div()
            .on_children_prepainted(move |children, window, _cx| {
                let Some(bounds) = children.first() else {
                    return;
                };
                apply_viewport(&shared, *bounds, window.scale_factor());
            })
            .id(root_id)
            .size_full();
        let root = custom_element_surface(root, &context).child(gpui::div().absolute().size_full());
        let root = match image {
            Some(image) => {
                let mut frame = gpui::img(image)
                    .absolute()
                    .size_full()
                    .object_fit(ObjectFit::Fill);
                if let Some(style) = context.style() {
                    if let Some(radius) = style.border_radius {
                        frame = frame.rounded(gpui::px(radius as f32));
                    }
                    if let Some(radius) = style.border_top_left_radius {
                        frame = frame.rounded_tl(gpui::px(radius as f32));
                    }
                    if let Some(radius) = style.border_top_right_radius {
                        frame = frame.rounded_tr(gpui::px(radius as f32));
                    }
                    if let Some(radius) = style.border_bottom_left_radius {
                        frame = frame.rounded_bl(gpui::px(radius as f32));
                    }
                    if let Some(radius) = style.border_bottom_right_radius {
                        frame = frame.rounded_br(gpui::px(radius as f32));
                    }
                }
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
        let Some(frame) = session.pump() else {
            return false;
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
            return false;
        }
        state.rejected_frame_dimensions = None;
        if state.frame_dimensions != Some((width, height)) {
            if verbose() {
                eprintln!("Photon Engine frame dimensions: {width}x{height}");
            }
            state.frame_dimensions = Some((width, height));
        }
        let pixels = tightly_packed_bgra(frame);
        match RenderImage::from_bgra(width as u32, height as u32, pixels) {
            Some(image) => {
                if verbose() {
                    let now = Instant::now();
                    let started = *state.frame_stats_started.get_or_insert(now);
                    state.frames_since_report += 1;
                    if now.duration_since(started) >= Duration::from_secs(5) {
                        let elapsed = now.duration_since(started).as_secs_f64();
                        eprintln!(
                            "Photon frames: {}x{} BGRA @ DPR {} ({:.1} frames/s)",
                            width,
                            height,
                            dpr,
                            state.frames_since_report as f64 / elapsed
                        );
                        state.frame_stats_started = Some(now);
                        state.frames_since_report = 0;
                    }
                }
                state.image = Some(Arc::new(image));
                true
            }
            None => {
                eprintln!("Photon frame had invalid BGRA dimensions");
                false
            }
        }
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

fn tightly_packed_bgra(frame: PresentedFrame) -> Vec<u8> {
    let row_bytes = frame.width as usize * 4;
    if frame.stride == row_bytes {
        return frame.pixels;
    }
    let mut pixels = Vec::with_capacity(row_bytes * frame.height as usize);
    for row in frame
        .pixels
        .chunks(frame.stride)
        .take(frame.height as usize)
    {
        pixels.extend_from_slice(&row[..row_bytes]);
    }
    pixels
}

fn verbose() -> bool {
    std::env::var_os("PHOTON_VERBOSE").is_some()
}
