//! Direct GPUI-CE application shell used by `./photon run`.
#![allow(deprecated)]

mod presentation_xpc;

use anyhow::Context as _;
use core_video::pixel_buffer::CVPixelBuffer;
use gpui::{
    App, Bounds, Context, Entity, ExternalMetalSurface, ExternalSurfaceDescriptor,
    ExternalTextureIdentity, FocusHandle, KeyDownEvent, KeyUpEvent, MetalSharedEventWait,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Render, ScrollDelta,
    ScrollWheelEvent, Subscription, SurfaceSource, TouchPhase, Window, WindowBounds, WindowOptions,
    div, prelude::*, px, size, surface,
};
use gpui_platform::application;
use io_surface::IOSurface;
use mach2::{port::mach_port_t, traps::mach_task_self};
use metal::SharedEvent;
use std::{
    collections::{HashMap, HashSet},
    ffi::{CStr, CString, c_char, c_void},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

mod embedder {
    use super::{c_char, c_void};
    unsafe extern "C" {
        pub fn photon_runtime_create(
            helper_directory: *const c_char,
            error: *mut c_char,
            capacity: usize,
        ) -> *mut c_void;
        pub fn photon_runtime_pump(runtime: *mut c_void);
        pub fn photon_runtime_destroy(runtime: *mut c_void);
        pub fn photon_view_create(
            runtime: *mut c_void,
            width: i32,
            height: i32,
            dpr: f64,
            context: *mut c_void,
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
            native_metal: bool,
            backing_callback: Option<
                unsafe extern "C" fn(*mut c_void, u64, u64, u32, u32, u32, u32) -> bool,
            >,
            native_frame_callback: Option<
                unsafe extern "C" fn(*mut c_void, u64, u64, u64, u64, i32, i32, f64),
            >,
        ) -> *mut c_void;
        pub fn photon_view_resize(view: *mut c_void, width: i32, height: i32, dpr: f64);
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
        pub fn photon_view_release_native_frame(
            view: *mut c_void,
            backing: u64,
            generation: u64,
            frame: u64,
        );
        pub fn photon_view_set_native_metal_presentation(view: *mut c_void, enabled: bool) -> bool;
        pub fn photon_view_navigate(view: *mut c_void, url: *const c_char);
        pub fn photon_view_shutdown(view: *mut c_void);
        pub fn photon_view_destroy(view: *mut c_void);
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct FrameKey {
    backing: u64,
    generation: u64,
    frame: u64,
}

#[derive(Clone, Copy)]
struct FrameReady {
    key: FrameKey,
    signal: u64,
    width: i32,
    height: i32,
}

#[derive(Clone, Copy)]
struct Release {
    key: FrameKey,
}

#[derive(Default)]
struct LeaseLedger {
    submitted: HashSet<FrameKey>,
    completed: HashSet<FrameKey>,
    released: HashSet<FrameKey>,
    pending: Vec<Release>,
}

impl LeaseLedger {
    fn submit(&mut self, key: FrameKey) {
        assert!(
            self.submitted.insert(key),
            "duplicate native frame lease {key:?}"
        );
    }

    fn complete(&mut self, key: FrameKey) {
        if !self.released.contains(&key) && self.completed.insert(key) {
            self.pending.push(Release { key });
        }
    }

    fn take_pending(&mut self) -> Vec<Release> {
        std::mem::take(&mut self.pending)
    }

    fn release(&mut self, key: FrameKey) {
        assert!(
            self.completed.contains(&key),
            "released incomplete frame lease {key:?}"
        );
        assert!(
            self.released.insert(key),
            "duplicate native frame release {key:?}"
        );
    }

    fn counts(&self) -> (usize, usize, usize, usize) {
        let outstanding = self.submitted.difference(&self.released).count();
        (
            self.submitted.len(),
            self.completed.len(),
            self.released.len(),
            outstanding,
        )
    }
}

#[derive(Default)]
struct GpuActivity {
    in_flight: Mutex<usize>,
    idle: Condvar,
}

impl GpuActivity {
    fn submitted(&self) {
        *self.in_flight.lock().unwrap() += 1;
    }

    fn completed(&self) {
        let mut in_flight = self.in_flight.lock().unwrap();
        assert!(
            *in_flight > 0,
            "Metal completion without a matching submission"
        );
        *in_flight -= 1;
        if *in_flight == 0 {
            self.idle.notify_all();
        }
    }

    fn wait_until_idle(&self) {
        let mut in_flight = self.in_flight.lock().unwrap();
        while *in_flight != 0 {
            in_flight = self.idle.wait(in_flight).unwrap();
        }
    }
}

struct PresentedSurface {
    surface: ExternalMetalSurface,
    key: FrameKey,
    releases_after_completion: Arc<Mutex<Vec<Release>>>,
}

struct Backing {
    image: CVPixelBuffer,
    iosurface_id: u32,
}

struct PresentationRuntime {
    channel: presentation_xpc::Channel,
    channel_id: String,
    consumer_event: Mutex<Option<SharedEvent>>,
    backings: Mutex<HashMap<(u64, u64), Backing>>,
    latest: Mutex<Option<FrameReady>>,
    latest_order: Mutex<(u64, u64)>,
    leases: Arc<Mutex<LeaseLedger>>,
    gpu_activity: Arc<GpuActivity>,
    accepting_frames: AtomicBool,
}

impl PresentationRuntime {
    fn new(
        service: &str,
        channel_id: String,
        leases: Arc<Mutex<LeaseLedger>>,
        gpu_activity: Arc<GpuActivity>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            channel: presentation_xpc::Channel::connect(service)?,
            channel_id,
            consumer_event: Mutex::new(None),
            backings: Mutex::new(HashMap::new()),
            latest: Mutex::new(None),
            latest_order: Mutex::new((0, 0)),
            leases,
            gpu_activity,
            accepting_frames: AtomicBool::new(true),
        })
    }

    fn activate(&self) -> anyhow::Result<()> {
        let device = gpui_apple::metal_renderer::MetalRenderer::selected_device();
        let (event, engine_registry_id) = self
            .channel
            .import_shared_event(&self.channel_id, &device)?;
        *self.consumer_event.lock().unwrap() = Some(event);
        trace(format_args!(
            "Metal registry ID match=yes engine={engine_registry_id} gpui_ce={}",
            device.registry_id()
        ));
        Ok(())
    }

    fn stop_accepting_frames(&self) {
        self.accepting_frames.store(false, Ordering::Release);
        if let Some(ready) = self.latest.lock().unwrap().take() {
            self.leases.lock().unwrap().complete(ready.key);
            trace(format_args!(
                "frame={} gen={} release=queued reason=shutdown-before-present",
                ready.key.frame, ready.key.generation
            ));
        }
    }

    fn register_backing(
        &self,
        backing: u64,
        generation: u64,
        width: u32,
        height: u32,
        pixel_format: u32,
        port: mach_port_t,
    ) -> anyhow::Result<()> {
        self.channel.register_backing(
            &self.channel_id,
            backing,
            generation,
            width,
            height,
            pixel_format,
            port,
        )?;
        let (imported_port, got_width, got_height, got_format) =
            self.channel
                .import_backing(&self.channel_id, backing, generation)?;
        let _port_guard = MachPortGuard(imported_port);
        anyhow::ensure!(
            got_width == width && got_height == height && got_format == pixel_format,
            "broker backing descriptor changed"
        );
        anyhow::ensure!(
            pixel_format == core_video::pixel_buffer::kCVPixelFormatType_32BGRA,
            "unsupported IOSurface pixel format {pixel_format:#x}"
        );
        #[allow(deprecated)]
        let raw_surface = unsafe { io_surface::IOSurfaceLookupFromMachPort(imported_port) };
        anyhow::ensure!(!raw_surface.is_null(), "IOSurfaceLookupFromMachPort failed");
        #[allow(deprecated)]
        let iosurface = IOSurface { obj: raw_surface };
        anyhow::ensure!(
            unsafe { io_surface::IOSurfaceGetWidth(raw_surface) } == width as usize,
            "IOSurface width differs from descriptor"
        );
        anyhow::ensure!(
            unsafe { io_surface::IOSurfaceGetHeight(raw_surface) } == height as usize,
            "IOSurface height differs from descriptor"
        );
        #[allow(deprecated)]
        let iosurface_id = unsafe { io_surface::IOSurfaceGetID(raw_surface) };
        let image = CVPixelBuffer::from_io_surface(&iosurface, None).map_err(|status| {
            anyhow::anyhow!("CVPixelBufferCreateWithIOSurface failed: {status}")
        })?;
        trace(format_args!(
            "backing={backing} gen={generation} surface={iosurface_id} cache=imported-once texture={width}x{height}"
        ));
        self.backings.lock().unwrap().insert(
            (backing, generation),
            Backing {
                image,
                iosurface_id,
            },
        );
        Ok(())
    }

    fn receive_frame(
        &self,
        backing: u64,
        generation: u64,
        frame: u64,
        signal: u64,
        width: i32,
        height: i32,
    ) {
        let key = FrameKey {
            backing,
            generation,
            frame,
        };
        self.leases.lock().unwrap().submit(key);
        if !self.accepting_frames.load(Ordering::Acquire) {
            self.leases.lock().unwrap().complete(key);
            trace(format_args!(
                "frame={frame} gen={generation} release=queued reason=shutting-down"
            ));
            return;
        }
        let order = (generation, frame);
        let mut latest_order = self.latest_order.lock().unwrap();
        if order <= *latest_order {
            trace(format_args!(
                "frame={frame} gen={generation} state=drop reason=out-of-order"
            ));
            self.leases.lock().unwrap().complete(key);
            return;
        }
        *latest_order = order;
        drop(latest_order);
        let ready = FrameReady {
            key: FrameKey {
                backing,
                generation,
                frame,
            },
            signal,
            width,
            height,
        };
        if let Some(replaced) = self.latest.lock().unwrap().replace(ready) {
            trace(format_args!(
                "frame={} gen={} state=release reason=superseded-before-present",
                replaced.key.frame, replaced.key.generation
            ));
            self.leases.lock().unwrap().complete(replaced.key);
        }
        trace(format_args!(
            "FrameReady frame={frame} gen={generation} backing={backing} signal={signal} content={width}x{height}"
        ));
    }

    fn take_surface(&self) -> Option<PresentedSurface> {
        let ready = self.latest.lock().unwrap().take()?;
        let backings = self.backings.lock().unwrap();
        let Some(backing) = backings.get(&(ready.key.backing, ready.key.generation)) else {
            self.leases.lock().unwrap().complete(ready.key);
            trace(format_args!(
                "frame={} gen={} release=queued reason=missing-backing",
                ready.key.frame, ready.key.generation
            ));
            return None;
        };
        if ready.width <= 0 || ready.height <= 0 {
            self.leases.lock().unwrap().complete(ready.key);
            trace(format_args!(
                "frame={} gen={} release=queued reason=invalid-content-size",
                ready.key.frame, ready.key.generation
            ));
            return None;
        }
        let Some(event) = self.consumer_event.lock().unwrap().as_ref().cloned() else {
            self.leases.lock().unwrap().complete(ready.key);
            trace(format_args!(
                "frame={} gen={} release=queued reason=missing-consumer-event",
                ready.key.frame, ready.key.generation
            ));
            return None;
        };
        if std::env::var_os("PHOTON_VERBOSE").is_some() {
            let producer_signaled = event.signaled_value();
            trace(format_args!(
                "frame={} gen={} producer-event signaled={} required={}",
                ready.key.frame, ready.key.generation, producer_signaled, ready.signal
            ));
        }
        let leases = self.leases.clone();
        let gpu_activity = self.gpu_activity.clone();
        let key = ready.key;
        let image = backing.image.clone();
        let releases_after_completion = Arc::new(Mutex::new(Vec::<Release>::new()));
        let completion_releases = releases_after_completion.clone();
        let surface = ExternalMetalSurface::new(
            ExternalSurfaceDescriptor {
                identity: ExternalTextureIdentity {
                    resource_id: key.backing,
                    generation: key.generation,
                    iosurface_id: backing.iosurface_id,
                },
                size: size(
                    gpui::DevicePixels(backing.image.get_width() as i32),
                    gpui::DevicePixels(backing.image.get_height() as i32),
                ),
                pixel_format: image.get_pixel_format(),
            },
            image,
            Some(MetalSharedEventWait {
                event,
                value: ready.signal,
            }),
            {
                let gpu_activity = gpu_activity.clone();
                move || gpu_activity.submitted()
            },
            move || {
                gpu_activity.completed();
                trace(format_args!(
                    "frame={} gen={} command-buffer=completed",
                    key.frame, key.generation
                ));
                let retired = std::mem::take(&mut *completion_releases.lock().unwrap());
                for release in retired {
                    leases.lock().unwrap().complete(release.key);
                    trace(format_args!(
                        "frame={} gen={} release=queued after-present={}",
                        release.key.frame, release.key.generation, key.frame
                    ));
                }
            },
        );
        trace(format_args!(
            "frame={} gen={} surface-ready signal={}",
            key.frame, key.generation, ready.signal
        ));
        Some(PresentedSurface {
            surface,
            key,
            releases_after_completion,
        })
    }
}

struct MachPortGuard(mach_port_t);
impl Drop for MachPortGuard {
    fn drop(&mut self) {
        unsafe { mach2::mach_port::mach_port_deallocate(mach_task_self(), self.0) };
    }
}

struct CallbackState {
    presentation: Arc<PresentationRuntime>,
}

struct EngineSession {
    runtime: *mut c_void,
    view: *mut c_void,
    callbacks: Box<CallbackState>,
    presentation: Arc<PresentationRuntime>,
    leases: Arc<Mutex<LeaseLedger>>,
    gpu_activity: Arc<GpuActivity>,
    final_surface_releases: Option<Arc<Mutex<Vec<Release>>>>,
    shutdown_started: bool,
    finished: bool,
}

impl EngineSession {
    fn create(width: i32, height: i32, dpr: f64) -> anyhow::Result<Self> {
        let helper = std::env::var("PHOTON_HELPER_DIRECTORY")
            .context("PHOTON_HELPER_DIRECTORY is not set")?;
        let helper = CString::new(helper)?;
        let service = std::env::var("PHOTON_PRESENTATION_XPC_SERVICE")
            .context("presentation broker service is not set")?;
        let channel_id = std::env::var("PHOTON_PRESENTATION_CHANNEL_ID")
            .context("presentation channel is not set")?;
        let leases = Arc::new(Mutex::new(LeaseLedger::default()));
        let gpu_activity = Arc::new(GpuActivity::default());
        let presentation = Arc::new(PresentationRuntime::new(
            &service,
            channel_id,
            leases.clone(),
            gpu_activity.clone(),
        )?);
        let mut error = [0_i8; 1024];
        let runtime = unsafe {
            embedder::photon_runtime_create(helper.as_ptr(), error.as_mut_ptr(), error.len())
        };
        anyhow::ensure!(
            !runtime.is_null(),
            "Photon Engine runtime failed: {}",
            unsafe { CStr::from_ptr(error.as_ptr()) }.to_string_lossy()
        );
        let mut callbacks = Box::new(CallbackState {
            presentation: presentation.clone(),
        });
        let view = unsafe {
            embedder::photon_view_create(
                runtime,
                width,
                height,
                dpr,
                (&mut *callbacks as *mut CallbackState).cast(),
                Some(on_engine_state),
                Some(on_engine_frame),
                Some(on_engine_cursor),
                Some(on_engine_error),
                true,
                Some(on_engine_backing),
                Some(on_engine_native_frame),
            )
        };
        if view.is_null() {
            unsafe { embedder::photon_runtime_destroy(runtime) };
            anyhow::bail!("Photon Engine could not create a webpage view");
        }
        let session = Self {
            runtime,
            view,
            callbacks,
            presentation: presentation.clone(),
            leases,
            gpu_activity,
            final_surface_releases: None,
            shutdown_started: false,
            finished: false,
        };
        if let Err(error) = presentation.activate() {
            drop(session);
            return Err(error);
        }
        if !unsafe { embedder::photon_view_set_native_metal_presentation(view, true) } {
            drop(session);
            anyhow::bail!("Photon Engine rejected native Metal presentation");
        }
        let url = std::env::var("PHOTON_URL").unwrap_or_else(|_| "https://example.com/".into());
        let url = photon_omnibox::resolve(&url)
            .map_err(|error| anyhow::anyhow!("invalid startup URL: {error:?}"))?
            .url()
            .to_owned();
        let url = CString::new(url)?;
        unsafe { embedder::photon_view_navigate(view, url.as_ptr()) };
        trace(format_args!(
            "Engine session ready; URL={}",
            url.to_string_lossy()
        ));
        Ok(session)
    }

    fn pump(&mut self) -> Option<PresentedSurface> {
        unsafe { embedder::photon_runtime_pump(self.runtime) };
        self.drain_releases();
        self.callbacks.presentation.take_surface()
    }

    fn resize(&mut self, width: i32, height: i32, dpr: f64) {
        unsafe { embedder::photon_view_resize(self.view, width, height, dpr) }
    }

    fn set_focus(&mut self, focused: bool) {
        unsafe { embedder::photon_view_set_focus(self.view, focused) }
    }

    #[allow(clippy::too_many_arguments)]
    fn send_pointer(
        &mut self,
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
    ) {
        unsafe {
            embedder::photon_view_pointer(
                self.view, kind, x, y, button, buttons, shift, control, alt, meta, wheel_x,
                wheel_y, precise, phase, clicks,
            )
        }
    }

    fn send_key(
        &mut self,
        key: u16,
        pressed: bool,
        code_point: u32,
        modifiers: gpui::Modifiers,
        repeat: bool,
        insert_text: bool,
    ) {
        unsafe {
            embedder::photon_view_key(
                self.view,
                key,
                pressed,
                code_point,
                modifiers.shift,
                modifiers.control,
                modifiers.alt,
                modifiers.platform,
                repeat,
                insert_text,
            )
        }
    }

    fn drain_releases(&mut self) {
        let pending = self.leases.lock().unwrap().take_pending();
        for release in pending {
            unsafe {
                embedder::photon_view_release_native_frame(
                    self.view,
                    release.key.backing,
                    release.key.generation,
                    release.key.frame,
                )
            };
            trace(format_args!(
                "FrameReleased frame={} gen={} delivered-to-engine",
                release.key.frame, release.key.generation
            ));
            self.leases.lock().unwrap().release(release.key);
        }
    }

    fn begin_shutdown(&mut self, final_surface_releases: Arc<Mutex<Vec<Release>>>) {
        if self.shutdown_started {
            return;
        }
        if !self.view.is_null() {
            unsafe {
                embedder::photon_view_set_native_metal_presentation(self.view, false);
            }
        }
        self.presentation.stop_accepting_frames();
        self.final_surface_releases = Some(final_surface_releases);
        self.shutdown_started = true;
    }

    fn finish_shutdown(&mut self) {
        if self.finished {
            return;
        }
        self.gpu_activity.wait_until_idle();
        if let Some(final_releases) = self.final_surface_releases.take() {
            for release in std::mem::take(&mut *final_releases.lock().unwrap()) {
                self.leases.lock().unwrap().complete(release.key);
            }
        }
        self.presentation.stop_accepting_frames();
        self.drain_releases();
        let (submitted, completed, released, outstanding) = self.leases.lock().unwrap().counts();
        trace(format_args!(
            "shutdown accounting submitted={submitted} completed={completed} released={released} outstanding={outstanding} gpu-in-flight=0"
        ));
        assert_eq!(
            outstanding, 0,
            "native presentation leases remain at shutdown"
        );
        assert_eq!(
            submitted, completed,
            "native presentation leases did not complete"
        );
        assert_eq!(
            completed, released,
            "completed native presentation leases were not released"
        );
        unsafe {
            if !self.view.is_null() {
                embedder::photon_view_shutdown(self.view);
                embedder::photon_view_destroy(self.view);
                self.view = std::ptr::null_mut();
            }
            if !self.runtime.is_null() {
                embedder::photon_runtime_destroy(self.runtime);
                self.runtime = std::ptr::null_mut();
            }
        }
        self.finished = true;
    }
}

impl Drop for EngineSession {
    fn drop(&mut self) {
        if !self.finished {
            self.begin_shutdown(Arc::new(Mutex::new(Vec::new())));
            self.finish_shutdown();
        }
    }
}

unsafe extern "C" fn on_engine_state(
    _: *mut c_void,
    url: *const c_char,
    _: *const c_char,
    loading: bool,
    _: bool,
    _: bool,
) {
    if !loading && !url.is_null() {
        trace(format_args!(
            "page-state url={}",
            unsafe { CStr::from_ptr(url) }.to_string_lossy()
        ));
    }
}
unsafe extern "C" fn on_engine_frame(
    _: *mut c_void,
    _: i32,
    _: i32,
    _: usize,
    _: f64,
    _: *const u8,
    _: usize,
    _: u64,
    _: u64,
    _: u64,
    _: u64,
) {
}
unsafe extern "C" fn on_engine_cursor(_: *mut c_void, _: i32) {}
unsafe extern "C" fn on_engine_error(_: *mut c_void, message: *const c_char) {
    if !message.is_null() {
        eprintln!(
            "Photon Engine: {}",
            unsafe { CStr::from_ptr(message) }.to_string_lossy()
        );
    }
}
unsafe extern "C" fn on_engine_backing(
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
    match state.presentation.register_backing(
        backing,
        generation,
        width,
        height,
        format,
        port as mach_port_t,
    ) {
        Ok(()) => true,
        Err(error) => {
            eprintln!("Photon presentation rejected backing: {error:#}");
            false
        }
    }
}
unsafe extern "C" fn on_engine_native_frame(
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
    unsafe { &*(context.cast::<CallbackState>()) }
        .presentation
        .receive_frame(backing, generation, frame, signal, width, height);
}

struct PhotonWebView {
    external: Option<PresentedSurface>,
    session: EngineSession,
    focus_handle: FocusHandle,
    last_viewport: Option<(i32, i32, u32)>,
    _quit_subscription: Option<Subscription>,
}

impl Drop for PhotonWebView {
    fn drop(&mut self) {
        self.prepare_shutdown();
    }
}

impl PhotonWebView {
    fn handle_mouse_up(&mut self, event: &MouseUpEvent) {
        let (button, _) = mouse_button(event.button);
        self.session.send_pointer(
            3,
            f64::from(f32::from(event.position.x)),
            f64::from(f32::from(event.position.y)),
            button,
            0,
            event.modifiers.shift,
            event.modifiers.control,
            event.modifiers.alt,
            event.modifiers.platform,
            0.0,
            0.0,
            false,
            0,
            event.click_count as i32,
        );
    }

    fn prepare_shutdown(&mut self) {
        if self.session.shutdown_started {
            return;
        }
        let final_releases = if let Some(presented) = self.external.as_ref() {
            let releases = presented.releases_after_completion.clone();
            let mut pending = releases.lock().unwrap();
            if !pending.iter().any(|release| release.key == presented.key) {
                pending.push(Release { key: presented.key });
            }
            drop(pending);
            releases
        } else {
            Arc::new(Mutex::new(Vec::new()))
        };
        self.session.begin_shutdown(final_releases);
    }

    fn new(cx: &mut Context<Self>) -> anyhow::Result<Self> {
        let width = 1200;
        let height = 760;
        Ok(Self {
            external: None,
            session: EngineSession::create(width, height, 1.0)?,
            focus_handle: cx.focus_handle(),
            last_viewport: None,
            _quit_subscription: None,
        })
    }
}

impl Render for PhotonWebView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bounds = window.bounds();
        let scale = window.scale_factor();
        let width = (f32::from(bounds.size.width) * scale).round() as i32;
        let height = (f32::from(bounds.size.height) * scale).round() as i32;
        let dpr_bits = scale.to_bits();
        let viewport = (width, height, dpr_bits);
        if self.last_viewport != Some(viewport) && width > 0 && height > 0 {
            self.session.resize(width, height, f64::from(scale));
            self.last_viewport = Some(viewport);
        }
        let mut webview = div()
            .size_full()
            .track_focus(&self.focus_handle)
            .on_any_mouse_down(cx.listener(|this, event: &MouseDownEvent, window, cx| {
                window.focus(&this.focus_handle, cx);
                this.session.set_focus(true);
                let (button, buttons) = mouse_button(event.button);
                this.session.send_pointer(
                    2,
                    f64::from(f32::from(event.position.x)),
                    f64::from(f32::from(event.position.y)),
                    button,
                    buttons,
                    event.modifiers.shift,
                    event.modifiers.control,
                    event.modifiers.alt,
                    event.modifiers.platform,
                    0.0,
                    0.0,
                    false,
                    0,
                    event.click_count as i32,
                );
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, event: &MouseUpEvent, _, _| this.handle_mouse_up(event)),
            )
            .on_mouse_up(
                MouseButton::Right,
                cx.listener(|this, event: &MouseUpEvent, _, _| this.handle_mouse_up(event)),
            )
            .on_mouse_up(
                MouseButton::Middle,
                cx.listener(|this, event: &MouseUpEvent, _, _| this.handle_mouse_up(event)),
            )
            .on_mouse_up(
                MouseButton::Navigate(gpui::NavigationDirection::Back),
                cx.listener(|this, event: &MouseUpEvent, _, _| this.handle_mouse_up(event)),
            )
            .on_mouse_up(
                MouseButton::Navigate(gpui::NavigationDirection::Forward),
                cx.listener(|this, event: &MouseUpEvent, _, _| this.handle_mouse_up(event)),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, _| {
                this.session.send_pointer(
                    0,
                    f64::from(f32::from(event.position.x)),
                    f64::from(f32::from(event.position.y)),
                    0,
                    event
                        .pressed_button
                        .map(mouse_button)
                        .map_or(0, |(_, buttons)| buttons),
                    event.modifiers.shift,
                    event.modifiers.control,
                    event.modifiers.alt,
                    event.modifiers.platform,
                    0.0,
                    0.0,
                    false,
                    0,
                    0,
                );
            }))
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, _| {
                let (delta, precise) = match event.delta {
                    ScrollDelta::Pixels(delta) => (
                        (f64::from(f32::from(delta.x)), f64::from(f32::from(delta.y))),
                        true,
                    ),
                    ScrollDelta::Lines(delta) => (
                        (f64::from(delta.x) * 40.0, f64::from(delta.y) * 40.0),
                        false,
                    ),
                };
                let phase = match event.touch_phase {
                    TouchPhase::Started | TouchPhase::Moved => 1,
                    TouchPhase::Ended | TouchPhase::Cancelled => 3,
                };
                this.session.send_pointer(
                    4,
                    f64::from(f32::from(event.position.x)),
                    f64::from(f32::from(event.position.y)),
                    0,
                    0,
                    event.modifiers.shift,
                    event.modifiers.control,
                    event.modifiers.alt,
                    event.modifiers.platform,
                    delta.0,
                    delta.1,
                    precise,
                    phase,
                    0,
                );
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, _| {
                let (key, code_point) =
                    keyboard_key(&event.keystroke.key, event.keystroke.key_char.as_deref());
                this.session.send_key(
                    key,
                    true,
                    code_point,
                    event.keystroke.modifiers,
                    event.is_held,
                    event.prefer_character_input && code_point != 0,
                );
            }))
            .on_key_up(cx.listener(|this, event: &KeyUpEvent, _, _| {
                let (key, code_point) =
                    keyboard_key(&event.keystroke.key, event.keystroke.key_char.as_deref());
                this.session.send_key(
                    key,
                    false,
                    code_point,
                    event.keystroke.modifiers,
                    false,
                    false,
                );
            }));
        if let Some(presented) = self.external.as_ref() {
            webview = webview.child(
                surface(SurfaceSource::ExternalMetal(presented.surface.clone())).size_full(),
            );
        }
        webview
    }
}

fn mouse_button(button: MouseButton) -> (i32, u8) {
    match button {
        MouseButton::Left => (1, 1),
        MouseButton::Right => (2, 2),
        MouseButton::Middle => (4, 4),
        MouseButton::Navigate(gpui::NavigationDirection::Back) => (8, 8),
        MouseButton::Navigate(gpui::NavigationDirection::Forward) => (16, 16),
    }
}

fn keyboard_key(key: &str, key_char: Option<&str>) -> (u16, u32) {
    let code_point = key_char
        .and_then(|text| text.chars().next())
        .map(u32::from)
        .unwrap_or(0);
    let key_code = match key.to_ascii_lowercase().as_str() {
        "backspace" => 0x08,
        "tab" => 0x09,
        "enter" | "return" => 0x0d,
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
        key if key.len() == 1 && key.as_bytes()[0].is_ascii_alphanumeric() => {
            u16::from(key.as_bytes()[0].to_ascii_uppercase())
        }
        key if key.strip_prefix('f').is_some_and(|n| {
            n.parse::<u8>()
                .is_ok_and(|number| (1..=12).contains(&number))
        }) =>
        {
            0x70 + key[1..].parse::<u16>().unwrap_or(1) - 1
        }
        _ => 0,
    };
    (key_code, code_point)
}

struct BrowserWindow {
    webview: Entity<PhotonWebView>,
}
impl Render for BrowserWindow {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.webview.clone())
    }
}

pub fn run() {
    application().run(|cx: &mut App| {
        let webview = cx.new(|cx| {
            let mut webview = PhotonWebView::new(cx)
                .unwrap_or_else(|error| panic!("could not start direct PhotonWebView: {error:#}"));
            let gpu_activity = webview.session.gpu_activity.clone();
            webview._quit_subscription = Some(cx.on_app_quit(
                move |view: &mut PhotonWebView, _cx: &mut Context<'_, PhotonWebView>| {
                    view.prepare_shutdown();
                    gpu_activity.wait_until_idle();
                    view.session.finish_shutdown();
                    async {}
                },
            ));
            webview
        });
        let window_size = size(px(1200.0), px(760.0));
        let bounds = Bounds::centered(None, window_size, cx);
        if let Ok(seconds) = std::env::var("PHOTON_SHUTDOWN_AFTER_SECONDS")
            && let Ok(seconds) = seconds.parse::<u64>()
            && seconds > 0
        {
            let timer = cx.background_executor().timer(Duration::from_secs(seconds));
            cx.spawn(async move |cx| {
                timer.await;
                let _ = cx.update(|cx| cx.quit());
            })
            .detach();
        }
        let webview_for_pump = webview.clone();
        cx.spawn(async move |cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let _ = webview_for_pump.update(cx, |view, cx| {
                    let next = view.session.pump();
                    let has_next = next.is_some();
                    if let Some(presented) = next {
                        if let Some(retired) = view.external.take() {
                            let pending = std::mem::take(
                                &mut *retired.releases_after_completion.lock().unwrap(),
                            );
                            let mut next_releases =
                                presented.releases_after_completion.lock().unwrap();
                            next_releases.push(Release { key: retired.key });
                            next_releases.extend(pending);
                        }
                        view.external = Some(presented);
                    }
                    if has_next {
                        cx.notify();
                    }
                });
            }
        })
        .detach();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| BrowserWindow { webview }),
        )
        .expect("open GPUI-CE Photon window");
    });
}

fn trace(args: std::fmt::Arguments<'_>) {
    if std::env::var_os("PHOTON_VERBOSE").is_some() {
        eprintln!("[PhotonWebView/GPUI-CE] {args}");
    }
}
