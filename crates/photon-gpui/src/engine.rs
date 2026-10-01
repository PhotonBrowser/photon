//! Photon Engine web surface registered with the GPUIX native renderer.

#[cfg(target_os = "macos")]
use gpui::AppContext;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char, c_void};
use std::ptr;
use std::rc::Rc;
#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
fn external_image_lease_trace_enabled() -> bool {
    std::env::var_os("EXTERNAL_IMAGE_LEASE_TRACE").is_some()
}

#[cfg(target_os = "macos")]
fn trace_external_image_lease(args: std::fmt::Arguments<'_>) {
    if !external_image_lease_trace_enabled() {
        return;
    }
    static TRACE_EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let elapsed_ns = TRACE_EPOCH.get_or_init(Instant::now).elapsed().as_nanos();
    // Format first so helper-process stderr cannot split an event's IDs and
    // timestamp across the formatter's individual writes.
    use std::io::Write;
    let unix_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let message = format!(
        "[ExternalImageLease][Photon] process_elapsed_ns={elapsed_ns} unix_ns={unix_ns} thread={:?} {args}\n",
        std::thread::current().id()
    );
    let _ = std::io::stderr().lock().write_all(message.as_bytes());
}

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
        #[cfg(target_os = "macos")]
        pub fn photon_runtime_set_native_release_drain_callback(
            runtime: *mut c_void,
            callback_data: *mut c_void,
            callback: Option<unsafe extern "C" fn(*mut c_void)>,
        );
        #[cfg(target_os = "macos")]
        pub fn photon_runtime_schedule_native_release_drain(runtime: *mut c_void);
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
            #[cfg(target_os = "macos")] native_metal_presentation: bool,
            #[cfg(target_os = "macos")] native_backing_callback: Option<
                unsafe extern "C" fn(*mut c_void, u64, u64, u32, u32, u32, u32) -> bool,
            >,
            #[cfg(target_os = "macos")] native_frame_callback: Option<
                unsafe extern "C" fn(*mut c_void, u64, u64, u64, u64, i32, i32, f64),
            >,
        ) -> *mut c_void;
        pub fn photon_view_resize(view: *mut c_void, width: i32, height: i32, dpr: f64);
        #[cfg(target_os = "macos")]
        pub fn photon_view_release_native_frame(
            view: *mut c_void,
            backing_id: u64,
            generation: u64,
            frame_id: u64,
        );
        #[cfg(target_os = "macos")]
        pub fn photon_view_set_native_metal_presentation(view: *mut c_void, enabled: bool) -> bool;
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
    #[cfg(target_os = "macos")]
    native_presentation: Mutex<Option<Arc<MacNativePresentation>>>,
    #[cfg(target_os = "macos")]
    latest_native_frame: Mutex<Option<gpui::MacExternalImageFrame>>,
    #[cfg(target_os = "macos")]
    latest_native_order: Mutex<NativeFrameOrder>,
    #[cfg(target_os = "macos")]
    latest_native_ready_at: Mutex<Option<(u64, u64, Instant)>>,
    #[cfg(target_os = "macos")]
    wake_app: Mutex<Option<gpui::AsyncApp>>,
    #[cfg(target_os = "macos")]
    wake_view: Mutex<Option<gpui::WeakEntity<gpuix_native::GpuixView>>>,
    #[cfg(target_os = "macos")]
    wake_window: Mutex<Option<gpui::AnyWindowHandle>>,
    #[cfg(target_os = "macos")]
    wake_metrics: Arc<Mutex<WakeMetrics>>,
    #[cfg(target_os = "macos")]
    pump_active: AtomicBool,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
struct NativeFrameOrder {
    generation: u64,
    frame_id: u64,
}

#[cfg(target_os = "macos")]
#[derive(Default)]
struct WakeMetrics {
    immediate: u64,
    borrowed: u64,
    other_fallback: u64,
    timings_ms: Vec<(&'static str, f64)>,
}

#[cfg(target_os = "macos")]
impl CallbackState {
    fn request_gpui_wake(&self) -> bool {
        self.request_gpui_wake_for_frame(0, Instant::now())
    }

    fn request_gpui_wake_for_frame(&self, frame_id: u64, ready_at: Instant) -> bool {
        trace_external_image_lease(format_args!("frame_id={frame_id} state=WAKE_REQUESTED"));
        if self.pump_active.load(Ordering::Acquire) {
            trace_external_image_lease(format_args!(
                "frame_id={frame_id} state=WAKE_QUEUED_DURING_ENGINE_PUMP"
            ));
        }
        let app = self.wake_app.lock().unwrap().clone();
        let view = self.wake_view.lock().unwrap().clone();
        let window = *self.wake_window.lock().unwrap();
        let wake_metrics = self.wake_metrics.clone();
        match (app, view) {
            (Some(mut app), Some(view)) => {
                // Engine callbacks run on the owning foreground thread. Try
                // the window's fallible update first so a ready frame can make
                // this display opportunity. It refuses reentrant App borrows;
                // callbacks during a draw take the queued path below instead.
                if let Some(window) = window {
                    let result = app.update_window(window, |_, window, cx| {
                        let result = view.update(cx, |_, cx| cx.notify());
                        window.refresh();
                        if frame_id != 0 {
                            let mut metrics = wake_metrics.lock().unwrap();
                            metrics.immediate += 1;
                            metrics.timings_ms.push((
                                "frame ready to window invalidation",
                                ready_at.elapsed().as_secs_f64() * 1000.0,
                            ));
                        }
                        trace_external_image_lease(format_args!(
                            "frame_id={frame_id} state=GPUI_WINDOW_REFRESH dispatch=immediate notify_result={result:?}"
                        ));
                    });
                    if result.is_ok() {
                        return true;
                    }
                    if let Err(error) = &result {
                        let mut metrics = wake_metrics.lock().unwrap();
                        if error.downcast_ref::<std::cell::BorrowMutError>().is_some() {
                            metrics.borrowed += 1;
                        } else {
                            metrics.other_fallback += 1;
                        }
                    }
                    trace_external_image_lease(format_args!(
                        "frame_id={frame_id} state=WAKE_DEFERRED result={result:?}"
                    ));
                }
                app.spawn(async move |app| {
                    trace_external_image_lease(format_args!(
                        "frame_id={frame_id} state=WAKE_DISPATCHED_TO_GPUI_THREAD"
                    ));
                    if let Some(window) = window {
                        let result = app.update_window(window, |_, window, cx| {
                            let result = view.update(cx, |_, cx| cx.notify());
                            window.refresh();
                            if frame_id != 0 {
                                wake_metrics.lock().unwrap().timings_ms.push((
                                    "frame ready to window invalidation",
                                    ready_at.elapsed().as_secs_f64() * 1000.0,
                                ));
                            }
                            trace_external_image_lease(format_args!(
                                "frame_id={frame_id} state=GPUI_WINDOW_REFRESH dispatch=queued notify_result={result:?}"
                            ));
                        });
                        trace_external_image_lease(format_args!(
                            "frame_id={frame_id} state=WAKE_UPDATE_RETURN result={result:?}"
                        ));
                    } else {
                        wake_metrics.lock().unwrap().other_fallback += 1;
                        let result = view.update(app, |_, cx| cx.notify());
                        trace_external_image_lease(format_args!(
                            "frame_id={frame_id} state=WAKE_UPDATE_RETURN result={result:?}"
                        ));
                    }
                })
                .detach();
                true
            }
            (Some(app), None) => {
                app.spawn(async move |app| app.refresh()).detach();
                true
            }
            (None, _) => {
                trace_external_image_lease(format_args!(
                    "frame_id={frame_id} state=WAKE_UNAVAILABLE reason=gpui_app_not_registered"
                ));
                false
            }
        }
    }
}

#[cfg(target_os = "macos")]
struct NativeReleaseQueue {
    pending: Mutex<Vec<(u64, u64, u64, Instant)>>,
    runtime: AtomicPtr<c_void>,
    drain_scheduled: AtomicBool,
}

#[cfg(target_os = "macos")]
impl NativeReleaseQueue {
    fn schedule(&self) {
        if self.drain_scheduled.swap(true, Ordering::AcqRel) {
            trace_external_image_lease(format_args!("state=RELEASE_WAKE_COALESCED"));
            return;
        }
        let runtime = self.runtime.load(Ordering::Acquire);
        if runtime.is_null() {
            self.drain_scheduled.store(false, Ordering::Release);
            trace_external_image_lease(format_args!(
                "state=RELEASE_WAKE_SKIPPED reason=runtime_unavailable"
            ));
            return;
        }
        trace_external_image_lease(format_args!("state=RELEASE_OWNER_WAKE_SCHEDULED"));
        unsafe { embedder::photon_runtime_schedule_native_release_drain(runtime) };
    }
}

#[cfg(target_os = "macos")]
struct MacNativePresentation {
    channel: Arc<gpui_apple::presentation_xpc::MacPresentationEventChannel>,
    channel_id: String,
    consumer_event: Mutex<Option<metal::SharedEvent>>,
    backings: Mutex<HashMap<(u64, u64), core_video::pixel_buffer::CVPixelBuffer>>,
    release_queue: Arc<NativeReleaseQueue>,
    outstanding_leases: Arc<(Mutex<usize>, Condvar)>,
    view: AtomicPtr<c_void>,
}

#[cfg(target_os = "macos")]
impl MacNativePresentation {
    fn create() -> Result<Self, String> {
        let service = std::env::var("PHOTON_PRESENTATION_XPC_SERVICE")
            .map_err(|_| "PHOTON_PRESENTATION_XPC_SERVICE is not set".to_string())?;
        let channel_id = std::env::var("PHOTON_PRESENTATION_CHANNEL_ID")
            .map_err(|_| "PHOTON_PRESENTATION_CHANNEL_ID is not set".to_string())?;
        let identity = gpui_apple::metal_renderer::MetalRenderer::device_identity();
        let device = metal::Device::system_default()
            .ok_or_else(|| "Metal has no system default device".to_string())?;
        if identity.registry_id != device.registry_id() {
            return Err(format!(
                "GPUI device changed during presentation setup: renderer={} system={}",
                identity.registry_id,
                device.registry_id()
            ));
        }
        let channel = Arc::new(
            gpui_apple::presentation_xpc::MacPresentationEventChannel::connect(&service).map_err(
                |error| format!("could not connect to presentation XPC service: {error:#}"),
            )?,
        );
        if verbose() {
            eprintln!(
                "Photon presentation: native Metal\nEngine GPU: awaiting compositor event\nGPUI GPU: {}\nGPUI registry ID: {}\nBackings: persistent per generation\nCPU frame copies: 0",
                identity.name, identity.registry_id
            );
        }
        Ok(Self {
            channel,
            channel_id,
            consumer_event: Mutex::new(None),
            backings: Mutex::new(HashMap::new()),
            release_queue: Arc::new(NativeReleaseQueue {
                pending: Mutex::new(Vec::new()),
                runtime: AtomicPtr::new(ptr::null_mut()),
                drain_scheduled: AtomicBool::new(false),
            }),
            outstanding_leases: Arc::new((Mutex::new(0), Condvar::new())),
            view: AtomicPtr::new(ptr::null_mut()),
        })
    }

    fn activate(&self) -> Result<(), String> {
        let device = metal::Device::system_default()
            .ok_or_else(|| "Metal has no system default device".to_string())?;
        let mut event = self.consumer_event.lock().unwrap();
        if event.is_some() {
            return Ok(());
        }
        let (imported, producer_registry_id) = self
            .channel
            .import_shared_event_with_identity(&self.channel_id, &device)
            .map_err(|error| format!("could not import compositor shared event: {error:#}"))?;
        if verbose() {
            eprintln!(
                "Engine compositor registry ID: {producer_registry_id}\nGPUI registry ID: {}\nMetal registry ID match: yes",
                device.registry_id()
            );
        }
        *event = Some(imported);
        Ok(())
    }

    fn register_backing(
        &self,
        backing_id: u64,
        generation: u64,
        width: u32,
        height: u32,
        pixel_format: u32,
        iosurface_port: u32,
    ) -> Result<(), String> {
        let descriptor = gpui::MacGpuBackingDescriptor {
            backing_id,
            generation,
            width,
            height,
            pixel_format,
            iosurface_port: unsafe {
                gpui::MacIOSurfaceSendRight::from_owned_raw(iosurface_port as _)
            },
        };
        self.channel
            .register_iosurface_backing(&self.channel_id, &descriptor)
            .map_err(|error| format!("could not register IOSurface backing: {error:#}"))?;
        let imported = self
            .channel
            .import_iosurface_backing(&self.channel_id, backing_id, generation)
            .map_err(|error| {
                format!("could not retrieve registered IOSurface backing: {error:#}")
            })?;
        let pixel_buffer =
            gpui_apple::metal_renderer::MetalRenderer::import_iosurface_backing(&imported)
                .map_err(|error| format!("could not import Ladybird IOSurface: {error:#}"))?;
        if pixel_buffer.get_width() != width as usize
            || pixel_buffer.get_height() != height as usize
        {
            return Err(format!(
                "registered IOSurface dimensions mismatch: expected {width}x{height}, got {}x{}",
                pixel_buffer.get_width(),
                pixel_buffer.get_height()
            ));
        }
        self.backings
            .lock()
            .unwrap()
            .insert((backing_id, generation), pixel_buffer);
        trace_external_image_lease(format_args!(
            "backing_id={backing_id} generation={generation} width={width} height={height} state=BACKING_IMPORTED"
        ));
        Ok(())
    }

    fn create_frame(
        self: &Arc<Self>,
        backing_id: u64,
        generation: u64,
        frame_id: u64,
        signal_value: u64,
        width: i32,
        height: i32,
    ) -> Result<gpui::MacExternalImageFrame, String> {
        let image_buffer = self
            .backings
            .lock()
            .unwrap()
            .get(&(backing_id, generation))
            .cloned()
            .ok_or_else(|| "frame references an unregistered IOSurface backing".to_string())?;
        if signal_value == 0 {
            return Err("compositor frame has no Metal producer signal".to_string());
        }
        let consumer_event = self
            .consumer_event
            .lock()
            .unwrap()
            .as_ref()
            .cloned()
            .ok_or_else(|| "compositor shared event was not imported".to_string())?;
        let iosurface_identity =
            gpui_apple::metal_renderer::MetalRenderer::iosurface_identity(&image_buffer)
                .map_err(|error| format!("could not identify imported IOSurface: {error:#}"))?;
        let release_queue = Arc::clone(&self.release_queue);
        let outstanding = self.outstanding_leases.clone();
        *outstanding.0.lock().unwrap() += 1;
        trace_external_image_lease(format_args!(
            "backing_id={backing_id} generation={generation} frame_id={frame_id} signal_value={signal_value} state=FRAME_CREATED outstanding_leases={}",
            *outstanding.0.lock().unwrap()
        ));
        Ok(gpui::MacExternalImageFrame::new_with_content_size(
            backing_id,
            generation,
            frame_id,
            iosurface_identity,
            image_buffer.get_pixel_format(),
            image_buffer,
            gpui::size(gpui::DevicePixels(width), gpui::DevicePixels(height)),
            consumer_event,
            signal_value,
            move || {
                let mut pending = release_queue.pending.lock().unwrap();
                pending.push((backing_id, generation, frame_id, Instant::now()));
                trace_external_image_lease(format_args!(
                    "backing_id={backing_id} generation={generation} frame_id={frame_id} state=LEASE_RELEASED->RELEASE_QUEUED pending_engine_releases={}",
                    pending.len()
                ));
                drop(pending);
                release_queue.schedule();
                let mut count = outstanding.0.lock().unwrap();
                *count = count
                    .checked_sub(1)
                    .expect("native frame lease released twice");
                outstanding.1.notify_all();
            },
        ))
    }

    fn release_unsubmitted_frame(&self, backing_id: u64, generation: u64, frame_id: u64) {
        self.release_queue.pending.lock().unwrap().push((
            backing_id,
            generation,
            frame_id,
            Instant::now(),
        ));
        self.release_queue.schedule();
    }

    fn wait_for_all_leases(&self) {
        let mut count = self.outstanding_leases.0.lock().unwrap();
        while *count != 0 {
            count = self.outstanding_leases.1.wait(count).unwrap();
        }
    }

    fn drain_engine_releases(&self) {
        let view = self.view.load(Ordering::Acquire);
        if view.is_null() {
            trace_external_image_lease(format_args!(
                "state=RELEASE_DRAIN_SKIPPED reason=view_null"
            ));
            return;
        }
        loop {
            let releases = std::mem::take(&mut *self.release_queue.pending.lock().unwrap());
            trace_external_image_lease(format_args!(
                "state=OWNER_RELEASE_DRAIN queued={}",
                releases.len()
            ));
            for (backing_id, generation, frame_id, queued_at) in releases {
                trace_external_image_lease(format_args!(
                    "backing_id={backing_id} generation={generation} frame_id={frame_id} state=RELEASE_SENT_TO_EMBEDDER queue_latency_us={}",
                    queued_at.elapsed().as_micros()
                ));
                unsafe {
                    embedder::photon_view_release_native_frame(
                        view, backing_id, generation, frame_id,
                    )
                };
            }
            self.release_queue
                .drain_scheduled
                .store(false, Ordering::Release);
            if self.release_queue.pending.lock().unwrap().is_empty()
                || self
                    .release_queue
                    .drain_scheduled
                    .swap(true, Ordering::AcqRel)
            {
                break;
            }
        }
    }

    fn set_runtime(&self, runtime: *mut c_void) {
        self.release_queue.runtime.store(runtime, Ordering::Release);
    }
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
    wake_immediate: u64,
    wake_borrowed: u64,
    wake_other_fallback: u64,
}

impl CallbackMetrics {
    #[cfg(target_os = "macos")]
    fn record_native_received(&mut self, at: Instant) {
        self.received += 1;
        self.engine_completed += 1;
        if let Some(previous) = self.last_received {
            self.timings_ms.push((
                "B callback interval",
                at.duration_since(previous).as_secs_f64() * 1000.0,
            ));
        }
        self.last_received = Some(at);
    }

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
    wake_immediate: u64,
    wake_borrowed: u64,
    wake_other_fallback: u64,
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
            "Photon profile {width}x{height} DPR {dpr:.2}: FPS Engine={:.1} received={:.1} accepted={:.1} GPUI-presented={:.1}; frames Engine={} received={} accepted={} coalesced={} dropped={} redraws={} presented={} pumps={} native window wakes immediate={} borrowed={} other fallback={} RenderImage created={} dropped={} GPUI image uploads={} recreated={} callback Vec alloc={} bytes={} LiveImage Vec alloc={} bytes={} C++ frame Vec alloc={} bytes={} | {}",
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
            self.wake_immediate,
            self.wake_borrowed,
            self.wake_other_fallback,
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
        let p99 = values[((count as f64 * 0.99).ceil() as usize)
            .saturating_sub(1)
            .min(count - 1)]
        .1;
        let p50 = values[count / 2].1;
        let max = values[count - 1].1;
        output.push(format!(
            "{label}[n={count} avg={average:.2}ms p50={p50:.2}ms p95={p95:.2}ms p99={p99:.2}ms max={max:.2}ms]"
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
        #[cfg(target_os = "macos")]
        let native_presentation = if std::env::var_os("PHOTON_PRESENTATION_XPC_SERVICE").is_some() {
            match MacNativePresentation::create() {
                Ok(presentation) => Some(Arc::new(presentation)),
                Err(error) => {
                    if verbose() {
                        eprintln!("Photon presentation: CPU fallback — {error}");
                    }
                    None
                }
            }
        } else {
            if verbose() {
                eprintln!(
                    "Photon presentation: CPU fallback — presentation XPC service is not configured"
                );
            }
            None
        };
        #[cfg(target_os = "macos")]
        let () = {
            *session.callbacks.native_presentation.lock().unwrap() = native_presentation.clone();
        };
        #[cfg(target_os = "macos")]
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
                native_presentation.is_some(),
                Some(on_engine_native_backing),
                Some(on_engine_native_frame),
            )
        };
        #[cfg(not(target_os = "macos"))]
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
        #[cfg(target_os = "macos")]
        if let Some(native) = native_presentation {
            native.view.store(view, Ordering::Release);
            match native.activate() {
                Ok(()) => {
                    native.set_runtime(session.runtime);
                    unsafe {
                        embedder::photon_runtime_set_native_release_drain_callback(
                            session.runtime,
                            (&mut *session.callbacks as *mut CallbackState).cast(),
                            Some(on_engine_native_release_drain),
                        );
                    }
                    if !unsafe { embedder::photon_view_set_native_metal_presentation(view, true) } {
                        unsafe {
                            embedder::photon_runtime_set_native_release_drain_callback(
                                session.runtime,
                                ptr::null_mut(),
                                None,
                            );
                        }
                        native.set_runtime(ptr::null_mut());
                        return Err(
                            "Photon Engine rejected native Metal presentation enablement".into(),
                        );
                    }
                }
                Err(error) => {
                    if verbose() {
                        eprintln!("Photon presentation: CPU fallback — {error}");
                    }
                    unsafe { embedder::photon_view_set_native_metal_presentation(view, false) };
                }
            }
        }
        Ok(session)
    }

    fn pump(&mut self) -> Option<PresentedFrame> {
        #[cfg(target_os = "macos")]
        trace_external_image_lease(format_args!("state=ENGINE_SESSION_PUMP_BEGIN"));
        #[cfg(target_os = "macos")]
        self.callbacks.pump_active.store(true, Ordering::Release);
        unsafe { embedder::photon_runtime_pump(self.runtime) };
        #[cfg(target_os = "macos")]
        self.callbacks.pump_active.store(false, Ordering::Release);
        #[cfg(target_os = "macos")]
        if let Some(native) = self.callbacks.native_presentation.lock().unwrap().clone() {
            native.drain_engine_releases();
        }
        trace_external_image_lease(format_args!("state=ENGINE_SESSION_PUMP_END"));
        self.callbacks.latest_frame.lock().unwrap().take()
    }

    #[cfg(target_os = "macos")]
    fn set_wake_app(&self, app: gpui::AsyncApp) {
        *self.callbacks.wake_app.lock().unwrap() = Some(app);
    }

    #[cfg(target_os = "macos")]
    fn set_wake_view(&self, view: gpui::WeakEntity<gpuix_native::GpuixView>) {
        *self.callbacks.wake_view.lock().unwrap() = Some(view);
    }

    #[cfg(target_os = "macos")]
    fn set_wake_window(&self, window: gpui::AnyWindowHandle) {
        *self.callbacks.wake_window.lock().unwrap() = Some(window);
    }

    #[cfg(target_os = "macos")]
    fn take_native_frame(&self) -> Option<gpui::MacExternalImageFrame> {
        self.callbacks.latest_native_frame.lock().unwrap().take()
    }

    #[cfg(target_os = "macos")]
    fn take_native_ready_at(&self, generation: u64, frame_id: u64) -> Option<Instant> {
        self.callbacks
            .latest_native_ready_at
            .lock()
            .unwrap()
            .take()
            .and_then(|(ready_generation, ready_frame_id, at)| {
                (ready_generation == generation && ready_frame_id == frame_id).then_some(at)
            })
    }

    fn recycle_pixels(&self, pixels: Vec<u8>) {
        let mut reusable = self.callbacks.reusable_pixels.lock().unwrap();
        if pixels.capacity() > reusable.capacity() {
            *reusable = pixels;
        }
    }

    fn take_metrics(&self) -> CallbackMetrics {
        let mut metrics = std::mem::take(&mut *self.callbacks.metrics.lock().unwrap());
        #[cfg(target_os = "macos")]
        {
            let wakes = std::mem::take(&mut *self.callbacks.wake_metrics.lock().unwrap());
            metrics.wake_immediate = wakes.immediate;
            metrics.wake_borrowed = wakes.borrowed;
            metrics.wake_other_fallback = wakes.other_fallback;
            metrics.timings_ms.extend(wakes.timings_ms);
        }
        metrics
    }

    fn take_cursor(&self) -> Option<i32> {
        self.callbacks.latest_cursor.lock().unwrap().take()
    }

    fn resize(&mut self, width: i32, height: i32, dpr: f64) {
        unsafe { embedder::photon_view_resize(self.view, width, height, dpr) }
    }

    fn navigate(&mut self, input: &str) -> Result<(), String> {
        // The shell hands over what a person typed, and deciding what that means
        // happens here, once, on the Rust side of the boundary. The address field
        // therefore cannot claim one destination while the engine opens another.
        let url = match photon_omnibox::resolve(input) {
            Ok(target) => target.url().to_owned(),
            Err(error) => {
                return Err(match error {
                    photon_omnibox::OmniboxError::Empty => "nothing to open".to_owned(),
                    photon_omnibox::OmniboxError::InvalidAddress => {
                        "that address cannot be opened".to_owned()
                    }
                });
            }
        };
        #[cfg(target_os = "macos")]
        trace_external_image_lease(format_args!("state=SHELL_NAVIGATION_REQUESTED url={url}"));
        // Log the resolved address, not the text that was typed: a query becomes a
        // search URL here, and the URL is what has to be diagnosable later.
        if verbose() {
            eprintln!("Photon page: {url}");
        }
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
    #[cfg(target_os = "macos")]
    callbacks.request_gpui_wake();
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn on_engine_native_release_drain(context: *mut c_void) {
    if context.is_null() {
        return;
    }
    let callbacks = unsafe { &*(context.cast::<CallbackState>()) };
    if let Some(native) = callbacks.native_presentation.lock().unwrap().clone() {
        trace_external_image_lease(format_args!("state=OWNER_THREAD_RELEASE_DRAIN_CALLBACK"));
        native.drain_engine_releases();
    }
}

impl Drop for EngineSession {
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        self.callbacks.wake_app.lock().unwrap().take();
        #[cfg(target_os = "macos")]
        self.callbacks.wake_window.lock().unwrap().take();
        #[cfg(target_os = "macos")]
        if let Some(native) = self.callbacks.native_presentation.lock().unwrap().clone() {
            if let Some(frame) = self.callbacks.latest_native_frame.lock().unwrap().take() {
                frame.release_unsubmitted();
            }
            native.wait_for_all_leases();
            native.drain_engine_releases();
            native.set_runtime(ptr::null_mut());
            unsafe {
                embedder::photon_runtime_set_native_release_drain_callback(
                    self.runtime,
                    ptr::null_mut(),
                    None,
                );
            }
        }
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
    #[cfg(target_os = "macos")]
    callbacks.request_gpui_wake();
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn on_engine_native_backing(
    context: *mut c_void,
    backing_id: u64,
    generation: u64,
    width: u32,
    height: u32,
    pixel_format: u32,
    iosurface_port: u32,
) -> bool {
    if context.is_null() {
        return false;
    }
    let callbacks = unsafe { &*(context.cast::<CallbackState>()) };
    let native = callbacks.native_presentation.lock().unwrap().clone();
    let Some(native) = native else {
        let _owned_right =
            unsafe { gpui::MacIOSurfaceSendRight::from_owned_raw(iosurface_port as _) };
        return false;
    };
    match native.register_backing(
        backing_id,
        generation,
        width,
        height,
        pixel_format,
        iosurface_port,
    ) {
        Ok(()) => {
            if verbose() {
                eprintln!(
                    "Photon Metal backing registered: id={backing_id} generation={generation} size={width}x{height}"
                );
            }
            true
        }
        Err(error) => {
            eprintln!("Photon native Metal backing rejected: {error}");
            false
        }
    }
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn on_engine_native_frame(
    context: *mut c_void,
    backing_id: u64,
    generation: u64,
    frame_id: u64,
    signal_value: u64,
    width: i32,
    height: i32,
    _dpr: f64,
) {
    if context.is_null() {
        return;
    }
    let ready_at = Instant::now();
    let callbacks = unsafe { &*(context.cast::<CallbackState>()) };
    trace_external_image_lease(format_args!(
        "backing_id={backing_id} generation={generation} frame_id={frame_id} state=FRAME_RECEIVED"
    ));
    callbacks
        .metrics
        .lock()
        .unwrap()
        .record_native_received(ready_at);
    let native = callbacks.native_presentation.lock().unwrap().clone();
    let Some(native) = native else {
        return;
    };
    let order = NativeFrameOrder {
        generation,
        frame_id,
    };
    let mut latest_order = callbacks.latest_native_order.lock().unwrap();
    if order <= *latest_order {
        trace_external_image_lease(format_args!(
            "backing_id={backing_id} generation={generation} frame_id={frame_id} state=FRAME_REJECTED reason=out_of_order latest_generation={} latest_frame_id={}",
            latest_order.generation, latest_order.frame_id
        ));
        drop(latest_order);
        native.release_unsubmitted_frame(backing_id, generation, frame_id);
        return;
    }
    *latest_order = order;
    drop(latest_order);
    let frame = match native.create_frame(
        backing_id,
        generation,
        frame_id,
        signal_value,
        width,
        height,
    ) {
        Ok(frame) => frame,
        Err(error) => {
            eprintln!("Photon native Metal frame rejected: {error}");
            native.release_unsubmitted_frame(backing_id, generation, frame_id);
            return;
        }
    };
    trace_external_image_lease(format_args!(
        "backing_id={backing_id} generation={generation} frame_id={frame_id} signal_value={signal_value} state=FRAME_READY_RECEIVED"
    ));
    let mut latest = callbacks.latest_native_frame.lock().unwrap();
    let latest_order = callbacks.latest_native_order.lock().unwrap();
    let pending_is_newer = latest.as_ref().is_some_and(|pending| {
        NativeFrameOrder {
            generation: pending.generation,
            frame_id: pending.frame_id,
        } >= order
    });
    if order < *latest_order || pending_is_newer {
        trace_external_image_lease(format_args!(
            "backing_id={backing_id} generation={generation} frame_id={frame_id} state=FRAME_REJECTED reason=superseded_during_import latest_generation={} latest_frame_id={}",
            latest_order.generation, latest_order.frame_id
        ));
        drop(latest_order);
        drop(latest);
        frame.release_unsubmitted();
        return;
    }
    if let Some(superseded) = latest.replace(frame) {
        callbacks.metrics.lock().unwrap().coalesced += 1;
        trace_external_image_lease(format_args!(
            "backing_id={} generation={} frame_id={} state=SUPERSEDED_BEFORE_SUBMIT",
            superseded.backing_id, superseded.generation, superseded.frame_id
        ));
        superseded.release_unsubmitted();
        if verbose() {
            eprintln!("Photon native Metal frame coalesced before GPUI submission");
        }
    }
    *callbacks.latest_native_ready_at.lock().unwrap() = Some((generation, frame_id, ready_at));
    drop(latest_order);
    drop(latest);
    let wake_requested = callbacks.request_gpui_wake_for_frame(frame_id, ready_at);
    if verbose() {
        eprintln!(
            "[Photon] native frame ready id={frame_id} generation={generation}; GPUIX entity wake {}",
            if wake_requested {
                "requested"
            } else {
                "deferred to active render"
            }
        );
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
    #[cfg(target_os = "macos")]
    native_frame: Option<gpui::MacExternalImageFrame>,
    #[cfg(target_os = "macos")]
    last_accepted_native_order: NativeFrameOrder,
    cursor: gpui::CursorStyle,
    // Latest GPUI layout viewport, applied as soon as the element is painted.
    viewport: Option<(i32, i32, u32)>,
    // Last viewport sent to Engine. Layout can change faster than the Engine
    // pump, so only the newest size should cross the embedder boundary.
    applied_viewport: Option<(i32, i32, u32)>,
    bounds_origin: Option<(f32, f32)>,
    focus_subscription: Option<gpui::Subscription>,
    blur_subscription: Option<gpui::Subscription>,
    initial_focus_requested: bool,
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
        // Start Ladybird while the initial GPUI tree is being assembled. The
        // actual viewport is not available until paint, but creating the
        // runtime and WebView here lets process startup overlap window setup.
        let (session, creation_failed) = match EngineSession::create(1, 1, 1.0) {
            Ok(session) => (Some(session), false),
            Err(error) => {
                eprintln!("Photon Engine startup failed: {error}");
                (None, true)
            }
        };
        Box::new(PhotonWebViewElement {
            state: Rc::new(RefCell::new(WebViewState {
                session,
                creation_failed,
                ..WebViewState::default()
            })),
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

        #[cfg(target_os = "macos")]
        trace_external_image_lease(format_args!(
            "state=PHOTON_WEBVIEW_RENDER element_id={}",
            context.id()
        ));

        #[cfg(target_os = "macos")]
        if let Some(session) = self.state.borrow().session.as_ref() {
            session.set_wake_app(cx.to_async());
            session.set_wake_view(cx.weak_entity());
            session.set_wake_window(window.window_handle());
        }
        // Process one nonblocking batch during GPUI rendering. Core's macOS
        // CFRunLoop sources wake this same thread for subsequent IPC, timers
        // and notifier readiness, so an idle WebView does not need a poll task.
        self.poll();

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
        #[cfg(target_os = "macos")]
        let native_frame = state.native_frame.clone();
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
        let initial_focus_state = shared.clone();
        let initial_focus_handle = focus_handle.clone();
        let keydown_state = shared.clone();
        let keyup_state = shared.clone();
        let root = gpui::div()
            .on_painted(move |bounds, window, app| {
                // Create the engine view from the web surface's own painted
                // bounds. Waiting for a child prepaint callback can leave the
                // initial viewport unavailable until the first interaction.
                apply_viewport(&shared, bounds, window.scale_factor());
                let focus_on_open = {
                    let mut state = initial_focus_state.borrow_mut();
                    if state.initial_focus_requested {
                        false
                    } else {
                        state.initial_focus_requested = true;
                        true
                    }
                };
                if focus_on_open {
                    window.focus(&initial_focus_handle, app);
                }
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
        #[cfg(target_os = "macos")]
        let root = if let Some(frame) = native_frame {
            let corners = context.style().map_or_else(Corners::default, |style| {
                let radius = style.border_radius.unwrap_or_default();
                Corners {
                    top_left: (style.border_top_left_radius.unwrap_or(radius) as f32).into(),
                    top_right: (style.border_top_right_radius.unwrap_or(radius) as f32).into(),
                    bottom_right: (style.border_bottom_right_radius.unwrap_or(radius) as f32)
                        .into(),
                    bottom_left: (style.border_bottom_left_radius.unwrap_or(radius) as f32).into(),
                }
            });
            let surface = gpui::canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    if verbose() {
                        eprintln!(
                            "[PhotonWebView] GPUI scene includes native frame id={} generation={} bounds={}x{}",
                            frame.frame_id,
                            frame.generation,
                            bounds.size.width,
                            bounds.size.height
                        );
                    }
                    window.paint_external_surface(bounds, corners, frame.clone());
                },
            )
            .absolute()
            .size_full();
            root.child(surface)
        } else {
            match image {
                Some(image) => {
                    let corners = context.style().map_or_else(Corners::default, |style| {
                        let radius = style.border_radius.unwrap_or_default();
                        Corners {
                            top_left: (style.border_top_left_radius.unwrap_or(radius) as f32)
                                .into(),
                            top_right: (style.border_top_right_radius.unwrap_or(radius) as f32)
                                .into(),
                            bottom_right: (style.border_bottom_right_radius.unwrap_or(radius)
                                as f32)
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
            }
        };
        #[cfg(not(target_os = "macos"))]
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
        #[cfg(target_os = "macos")]
        if let Some(frame) = state.native_frame.take() {
            if frame.is_submitted() {
                frame.retire();
            } else {
                frame.release_unsubmitted();
            }
        }
        state.session.take();
        state.image.take();
    }

    fn live_dynamic_image(&self) -> Option<Arc<LiveImage>> {
        self.state.borrow().image.clone()
    }

    fn needs_polling(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            return false;
        }
        #[cfg(not(target_os = "macos"))]
        self.state.borrow().session.is_some()
    }

    fn poll(&mut self) -> bool {
        let mut state = self.state.borrow_mut();
        let pending_viewport = state
            .viewport
            .filter(|viewport| state.applied_viewport != Some(*viewport));
        if let Some((width, height, dpr_bits)) = pending_viewport {
            if let Some(session) = state.session.as_mut() {
                session.resize(width, height, f64::from(f32::from_bits(dpr_bits)));
                state.applied_viewport = Some((width, height, dpr_bits));
            }
        }
        let url = state
            .viewport
            .filter(|_| {
                !is_initial_blank_url(state.url.as_str())
                    && !state
                        .navigated_url
                        .as_deref()
                        .is_some_and(|url| equivalent_urls(url, state.url.as_str()))
            })
            .filter(|_| !state.url.is_empty())
            .map(|_| state.url.clone());
        if let Some(url) = url {
            let navigation = state.session.as_mut().map(|session| session.navigate(&url));
            match navigation {
                Some(Ok(())) => {
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
        state.diagnostics.wake_immediate += metrics.wake_immediate;
        state.diagnostics.wake_borrowed += metrics.wake_borrowed;
        state.diagnostics.wake_other_fallback += metrics.wake_other_fallback;
        state.diagnostics.timings_ms.extend(metrics.timings_ms);
        let mut cursor_changed = false;
        if let Some(cursor) = cursor
            && state.cursor != cursor
        {
            state.cursor = cursor;
            cursor_changed = true;
        }
        #[cfg(target_os = "macos")]
        let native_frame = state
            .session
            .as_ref()
            .and_then(EngineSession::take_native_frame);
        #[cfg(target_os = "macos")]
        if let Some(frame) = native_frame {
            let frame_order = NativeFrameOrder {
                generation: frame.generation,
                frame_id: frame.frame_id,
            };
            if frame_order <= state.last_accepted_native_order {
                trace_external_image_lease(format_args!(
                    "backing_id={} generation={} frame_id={} state=FRAME_REJECTED reason=out_of_order_consume latest_generation={} latest_frame_id={}",
                    frame.backing_id,
                    frame.generation,
                    frame.frame_id,
                    state.last_accepted_native_order.generation,
                    state.last_accepted_native_order.frame_id
                ));
                frame.release_unsubmitted();
                return false;
            }
            state.last_accepted_native_order = frame_order;
            if let Some(ready_at) = state
                .session
                .as_ref()
                .and_then(|session| session.take_native_ready_at(frame.generation, frame.frame_id))
            {
                state.diagnostics.timings_ms.push((
                    "frame ready to PhotonWebView consume",
                    ready_at.elapsed().as_secs_f64() * 1000.0,
                ));
            }
            trace_external_image_lease(format_args!(
                "backing_id={} generation={} frame_id={} state=GPUIX_CONSUMED",
                frame.backing_id, frame.generation, frame.frame_id
            ));
            if verbose() {
                eprintln!(
                    "[PhotonWebView] consuming native frame id={} generation={}",
                    frame.frame_id, frame.generation
                );
            }
            let dimensions = (frame.content_size.width.0, frame.content_size.height.0);
            let expected_dimensions = state.viewport.map(|(width, height, dpr_bits)| {
                let dpr = f64::from(f32::from_bits(dpr_bits));
                (
                    (f64::from(width) * dpr).round() as i32,
                    (f64::from(height) * dpr).round() as i32,
                )
            });
            if expected_dimensions != Some(dimensions) {
                if verbose() {
                    eprintln!(
                        "Photon Metal frame is {}x{}; waiting for viewport {}x{}",
                        dimensions.0,
                        dimensions.1,
                        expected_dimensions.map_or(0, |size| size.0),
                        expected_dimensions.map_or(0, |size| size.1)
                    );
                }
                frame.release_unsubmitted();
                return false;
            }
            state.rejected_frame_dimensions = None;
            if state.frame_dimensions != Some(dimensions) {
                state.frame_dimensions = Some(dimensions);
                if verbose() {
                    eprintln!(
                        "Photon native Metal frame dimensions: {}x{}",
                        dimensions.0, dimensions.1
                    );
                }
            }
            if let Some(previous) = state.native_frame.replace(frame) {
                trace_external_image_lease(format_args!(
                    "backing_id={} generation={} frame_id={} state=RETIRED_BY_NEW_FRAME submitted={}",
                    previous.backing_id,
                    previous.generation,
                    previous.frame_id,
                    previous.is_submitted()
                ));
                if previous.is_submitted() {
                    previous.retire();
                } else {
                    previous.release_unsubmitted();
                }
            }
            state.diagnostics.accepted += 1;
            state.diagnostics.redraw_requested += 1;
            state.diagnostics.redraw_requested_at = Some(Instant::now());
            if verbose() {
                state.diagnostics.presented += 1;
            }
            if verbose() {
                let (width, height, dpr) = profile_size(state.viewport, state.frame_dimensions);
                state.diagnostics.report_if_due(width, height, dpr);
            }
            return true;
        }
        #[cfg(target_os = "macos")]
        if frame.is_some()
            && let Some(previous) = state.native_frame.take()
        {
            if previous.is_submitted() {
                previous.retire();
            } else {
                previous.release_unsubmitted();
            }
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

    // Prime the newly created view with the real layout before its first
    // engine pump. Starting the initial navigation here lets loading begin in
    // the window's first frame with the correct viewport already queued.
    let mut state = state.borrow_mut();
    if let Some(viewport) = state.viewport {
        if state.applied_viewport.is_none()
            && let Some(session) = state.session.as_mut()
        {
            session.resize(viewport.0, viewport.1, f64::from(dpr));
            state.applied_viewport = Some(viewport);
        }
        let url = (state.navigated_url.as_deref() != Some(state.url.as_str())
            && !state.url.is_empty()
            && !is_initial_blank_url(&state.url)
            && !state
                .navigated_url
                .as_deref()
                .is_some_and(|url| equivalent_urls(url, state.url.as_str())))
        .then(|| state.url.clone());
        if let Some(url) = url
            && let Some(session) = state.session.as_mut()
        {
            match session.navigate(&url) {
                Ok(()) => {
                    state.navigated_url = Some(url);
                }
                Err(error) => eprintln!("Photon navigation failed: {error}"),
            }
        }
    }
}

fn verbose() -> bool {
    std::env::var_os("PHOTON_VERBOSE").is_some()
}

fn is_initial_blank_url(url: &str) -> bool {
    url.eq_ignore_ascii_case("about:blank")
}

fn equivalent_urls(left: &str, right: &str) -> bool {
    fn without_root_slash(url: &str) -> String {
        let Some((scheme, rest)) = url.split_once("://") else {
            return url.to_string();
        };
        let Some(authority) = rest.strip_suffix('/') else {
            return url.to_string();
        };
        if authority.contains('/') {
            return url.to_string();
        }
        format!("{scheme}://{authority}")
    }

    without_root_slash(left) == without_root_slash(right)
}

fn profile_size(viewport: Option<(i32, i32, u32)>, frame: Option<(i32, i32)>) -> (i32, i32, f64) {
    if let Some((width, height, dpr_bits)) = viewport {
        return (width, height, f64::from(f32::from_bits(dpr_bits)));
    }
    frame.map_or((0, 0, 1.0), |(width, height)| (width, height, 1.0))
}
