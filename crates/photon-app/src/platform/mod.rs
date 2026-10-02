//! Minimal, temporary direct GPUI-CE shell used to prove native frame presentation.
#![allow(deprecated)]

mod presentation_xpc;

use anyhow::Context as _;
use core_video::pixel_buffer::CVPixelBuffer;
use gpui::{
    App, Bounds, Context, Entity, ExternalMetalSurface, ExternalSurfaceDescriptor,
    ExternalTextureIdentity, MetalSharedEventWait, Render, SurfaceSource, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, size, surface,
};
use gpui_platform::application;
use io_surface::IOSurface;
use mach2::{port::mach_port_t, traps::mach_task_self};
use metal::SharedEvent;
use std::{
    collections::HashMap,
    ffi::{CStr, CString, c_char, c_void},
    sync::{Arc, Mutex},
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
    releases: Arc<Mutex<Vec<Release>>>,
}

impl PresentationRuntime {
    fn new(
        service: &str,
        channel_id: String,
        releases: Arc<Mutex<Vec<Release>>>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            channel: presentation_xpc::Channel::connect(service)?,
            channel_id,
            consumer_event: Mutex::new(None),
            backings: Mutex::new(HashMap::new()),
            latest: Mutex::new(None),
            latest_order: Mutex::new((0, 0)),
            releases,
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
        let order = (generation, frame);
        let mut latest_order = self.latest_order.lock().unwrap();
        if order <= *latest_order {
            trace(format_args!(
                "frame={frame} gen={generation} state=drop reason=out-of-order"
            ));
            self.releases.lock().unwrap().push(Release {
                key: FrameKey {
                    backing,
                    generation,
                    frame,
                },
            });
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
            self.releases
                .lock()
                .unwrap()
                .push(Release { key: replaced.key });
        }
        trace(format_args!(
            "FrameReady frame={frame} gen={generation} backing={backing} signal={signal} content={width}x{height}"
        ));
    }

    fn take_surface(&self) -> Option<PresentedSurface> {
        let ready = self.latest.lock().unwrap().take()?;
        let backings = self.backings.lock().unwrap();
        let Some(backing) = backings.get(&(ready.key.backing, ready.key.generation)) else {
            self.releases
                .lock()
                .unwrap()
                .push(Release { key: ready.key });
            trace(format_args!(
                "frame={} gen={} release=queued reason=missing-backing",
                ready.key.frame, ready.key.generation
            ));
            return None;
        };
        if ready.width <= 0 || ready.height <= 0 {
            self.releases
                .lock()
                .unwrap()
                .push(Release { key: ready.key });
            trace(format_args!(
                "frame={} gen={} release=queued reason=invalid-content-size",
                ready.key.frame, ready.key.generation
            ));
            return None;
        }
        let event = self.consumer_event.lock().unwrap().as_ref()?.clone();
        if std::env::var_os("PHOTON_VERBOSE").is_some() {
            let producer_signaled = event.signaled_value();
            trace(format_args!(
                "frame={} gen={} producer-event signaled={} required={}",
                ready.key.frame, ready.key.generation, producer_signaled, ready.signal
            ));
        }
        let releases = self.releases.clone();
        let key = ready.key;
        let image = backing.image.clone();
        let releases_after_completion = Arc::new(Mutex::new(Vec::new()));
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
            move || {
                trace(format_args!(
                    "frame={} gen={} command-buffer=completed",
                    key.frame, key.generation
                ));
                let retired = std::mem::take(&mut *completion_releases.lock().unwrap());
                for release in retired {
                    releases.lock().unwrap().push(release);
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
    releases: Arc<Mutex<Vec<Release>>>,
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
        let releases = Arc::new(Mutex::new(Vec::new()));
        let presentation = Arc::new(PresentationRuntime::new(
            &service,
            channel_id,
            releases.clone(),
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
            releases,
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

    fn drain_releases(&mut self) {
        let pending = std::mem::take(&mut *self.releases.lock().unwrap());
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
        }
    }
}

impl Drop for EngineSession {
    fn drop(&mut self) {
        self.drain_releases();
        unsafe {
            if !self.view.is_null() {
                embedder::photon_view_shutdown(self.view);
                embedder::photon_view_destroy(self.view);
            }
            if !self.runtime.is_null() {
                embedder::photon_runtime_destroy(self.runtime);
            }
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
    last_viewport: Option<(i32, i32, u32)>,
}

impl PhotonWebView {
    fn new() -> anyhow::Result<Self> {
        let width = 1200;
        let height = 760;
        Ok(Self {
            external: None,
            session: EngineSession::create(width, height, 1.0)?,
            last_viewport: None,
        })
    }
}

impl Render for PhotonWebView {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
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
        match self.external.as_ref() {
            Some(presented) => surface(SurfaceSource::ExternalMetal(presented.surface.clone()))
                .size_full()
                .into_any_element(),
            None => div().size_full().into_any_element(),
        }
    }
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
        let webview = match cx.new(|_| {
            PhotonWebView::new()
                .unwrap_or_else(|error| panic!("could not start direct PhotonWebView: {error:#}"))
        }) {
            webview => webview,
        };
        let window_size = size(px(1200.0), px(760.0));
        let bounds = Bounds::centered(None, window_size, cx);
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
