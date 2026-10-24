//! IOSurface frame acceptance, presentation, and lease bookkeeping.
#![allow(deprecated)]

use anyhow::Context as _;
use core_video::pixel_buffer::CVPixelBuffer;
use gpui::{
    ExternalMetalSurface, ExternalSurfaceDescriptor, ExternalTextureIdentity, MetalSharedEventWait,
    size,
};
use io_surface::IOSurface;
use mach2::{port::mach_port_t, traps::mach_task_self};
use metal::SharedEvent;
use photon_performance::PerformanceMonitor;
use std::{
    collections::HashMap,
    ffi::c_void,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering},
    },
    time::Instant,
};

use super::super::engine::navigation_timing::NavigationTimingProfiler;
use crate::platform::engine_ffi::embedder;
use crate::platform::trace;
use photon_presentation_ipc::{BackingDescriptor, Channel};

pub(in crate::platform) struct MachPortGuard(pub(in crate::platform) mach_port_t);
impl Drop for MachPortGuard {
    fn drop(&mut self) {
        unsafe { mach2::mach_port::mach_port_deallocate(mach_task_self(), self.0) };
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(in crate::platform) struct FrameKey {
    pub(in crate::platform) backing: u64,
    pub(in crate::platform) generation: u64,
    pub(in crate::platform) frame: u64,
}

#[derive(Clone, Copy)]
pub(in crate::platform) struct FrameReady {
    pub(in crate::platform) key: FrameKey,
    pub(in crate::platform) signal: u64,
    pub(in crate::platform) width: i32,
    pub(in crate::platform) height: i32,
}

#[derive(Clone, Copy)]
pub(in crate::platform) struct Release {
    pub(in crate::platform) key: FrameKey,
}

/// Where a frame lease the Engine handed over has got to.
#[derive(Default)]
struct Lease {
    accepted: bool,
    completed: bool,
}

/// The frame leases still held, and how many passed each stage. A lease is
/// forgotten once released, so the ledger stays as small as the frames in
/// flight however long a page plays.
#[derive(Default)]
pub(in crate::platform) struct LeaseLedger {
    held: HashMap<FrameKey, Lease>,
    counts: LeaseCounts,
    pending: Vec<Release>,
}

impl LeaseLedger {
    pub(in crate::platform) fn receive(&mut self, key: FrameKey) {
        assert!(
            self.held.insert(key, Lease::default()).is_none(),
            "duplicate native frame lease {key:?}"
        );
        self.counts.received += 1;
    }

    pub(in crate::platform) fn accept(&mut self, key: FrameKey) {
        let lease = self.held.get_mut(&key).expect("accepted an unknown frame");
        assert!(!lease.accepted, "duplicate accepted frame {key:?}");
        lease.accepted = true;
        self.counts.accepted += 1;
    }

    pub(in crate::platform) fn present(&mut self, key: FrameKey) {
        assert!(self.held.get(&key).is_some_and(|lease| lease.accepted));
        self.counts.presented += 1;
    }

    /// Queues a held lease's release, once.
    pub(in crate::platform) fn complete(&mut self, key: FrameKey) {
        if let Some(lease) = self.held.get_mut(&key)
            && !lease.completed
        {
            lease.completed = true;
            self.counts.completed += 1;
            self.pending.push(Release { key });
        }
    }

    pub(in crate::platform) fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    pub(in crate::platform) fn take_pending(&mut self) -> Vec<Release> {
        std::mem::take(&mut self.pending)
    }

    pub(in crate::platform) fn release(&mut self, key: FrameKey) {
        let lease = self
            .held
            .remove(&key)
            .unwrap_or_else(|| panic!("duplicate native frame release {key:?}"));
        assert!(lease.completed, "released incomplete frame lease {key:?}");
        self.counts.released += 1;
    }

    pub(in crate::platform) fn counts(&self) -> LeaseCounts {
        LeaseCounts {
            outstanding: self.held.len(),
            ..self.counts
        }
    }
}

#[derive(Clone, Copy, Default)]
pub(in crate::platform) struct LeaseCounts {
    pub(in crate::platform) received: usize,
    pub(in crate::platform) accepted: usize,
    pub(in crate::platform) presented: usize,
    pub(in crate::platform) completed: usize,
    pub(in crate::platform) released: usize,
    pub(in crate::platform) outstanding: usize,
}

#[derive(Default)]
pub(in crate::platform) struct GpuActivity {
    progress: Mutex<GpuProgress>,
    idle: Condvar,
}

#[derive(Default)]
pub(in crate::platform) struct GpuProgress {
    in_flight: usize,
    submitted: usize,
    completed: usize,
}

impl GpuActivity {
    pub(in crate::platform) fn submitted(&self) {
        let mut progress = self.progress.lock().unwrap();
        progress.in_flight += 1;
        progress.submitted += 1;
    }

    pub(in crate::platform) fn completed(&self) {
        let mut progress = self.progress.lock().unwrap();
        assert!(
            progress.in_flight > 0,
            "Metal completion without a matching submission"
        );
        progress.in_flight -= 1;
        progress.completed += 1;
        if progress.in_flight == 0 {
            self.idle.notify_all();
        }
    }

    pub(in crate::platform) fn wait_until_idle(&self) {
        let mut progress = self.progress.lock().unwrap();
        while progress.in_flight != 0 {
            progress = self.idle.wait(progress).unwrap();
        }
    }

    pub(in crate::platform) fn counts(&self) -> (usize, usize, usize) {
        let progress = self.progress.lock().unwrap();
        (progress.submitted, progress.completed, progress.in_flight)
    }
}

/// How a presented frame is being read by GPUI-CE command buffers.
#[derive(Default)]
pub(in crate::platform) struct Sampling {
    in_flight: AtomicUsize,
    /// Set once a command buffer sampling a newer frame has been submitted, so
    /// no command buffer submitted afterwards samples this one.
    superseded: AtomicBool,
}

/// A frame no longer displayed, released to Engine once nothing samples it.
pub(in crate::platform) struct RetiredSurface {
    pub(in crate::platform) key: FrameKey,
    sampling: Arc<Sampling>,
}

pub(in crate::platform) struct PresentedSurface {
    pub(in crate::platform) surface: ExternalMetalSurface,
    pub(in crate::platform) key: FrameKey,
    pub(in crate::platform) navigation_id: Option<u64>,
    sampling: Arc<Sampling>,
    /// Frames this one replaced; they are superseded when it is first submitted.
    pub(in crate::platform) retired_until_submitted: Arc<Mutex<Vec<RetiredSurface>>>,
}

impl PresentedSurface {
    pub(in crate::platform) fn timing_identity(&self) -> (u64, u64) {
        (self.key.generation, self.key.frame)
    }

    pub(in crate::platform) fn retire(&self) -> RetiredSurface {
        RetiredSurface {
            key: self.key,
            sampling: self.sampling.clone(),
        }
    }
}

pub(in crate::platform) struct Backing {
    image: CVPixelBuffer,
    iosurface_id: u32,
}

fn schedule_release_drain(release_scheduler: &AtomicPtr<c_void>) {
    let runtime = release_scheduler.load(Ordering::Acquire);
    if !runtime.is_null() {
        unsafe { embedder::photon_runtime_schedule_native_release_drain(runtime) };
    }
}

pub(in crate::platform) struct PresentationRuntime {
    channel: Channel,
    channel_id: String,
    consumer_event: Mutex<Option<SharedEvent>>,
    backings: Mutex<HashMap<(u64, u64), Backing>>,
    latest: Mutex<Option<FrameReady>>,
    latest_order: Mutex<(u64, u64)>,
    pub(in crate::platform) leases: Arc<Mutex<LeaseLedger>>,
    pub(in crate::platform) gpu_activity: Arc<GpuActivity>,
    pub(in crate::platform) release_scheduler: Arc<AtomicPtr<c_void>>,
    navigation_timing: Option<Arc<NavigationTimingProfiler>>,
    performance: Arc<PerformanceMonitor>,
    accepting_frames: AtomicBool,
}

impl PresentationRuntime {
    pub(in crate::platform) fn new(
        leases: Arc<Mutex<LeaseLedger>>,
        gpu_activity: Arc<GpuActivity>,
        navigation_timing: Option<Arc<NavigationTimingProfiler>>,
        performance: Arc<PerformanceMonitor>,
    ) -> anyhow::Result<Self> {
        let service = std::env::var("PHOTON_PRESENTATION_XPC_SERVICE")
            .context("presentation broker service is not set")?;
        let channel_id = presentation_channel_id();
        Ok(Self {
            channel: Channel::connect(&service)?,
            channel_id,
            consumer_event: Mutex::new(None),
            backings: Mutex::new(HashMap::new()),
            latest: Mutex::new(None),
            latest_order: Mutex::new((0, 0)),
            leases,
            gpu_activity,
            release_scheduler: Arc::new(AtomicPtr::new(std::ptr::null_mut())),
            navigation_timing,
            performance,
            accepting_frames: AtomicBool::new(true),
        })
    }

    pub(in crate::platform) fn activate(&self) -> anyhow::Result<()> {
        let device = gpui_apple::metal_renderer::MetalRenderer::selected_device();
        match self.channel.import_shared_event(&self.channel_id, &device) {
            Ok(Some((event, engine_registry_id))) => {
                *self.consumer_event.lock().unwrap() = Some(event);
                trace(format_args!(
                    "Metal registry ID match=yes engine={engine_registry_id} gpui_ce={}",
                    device.registry_id()
                ));
            }
            Ok(None) => trace(format_args!(
                "producer shared event unavailable; engine will publish frames after GPU completion"
            )),
            Err(error) => return Err(error),
        }
        Ok(())
    }

    pub(in crate::platform) fn stop_accepting_frames(&self) {
        self.accepting_frames.store(false, Ordering::Release);
        if let Some(ready) = self.latest.lock().unwrap().take() {
            self.leases.lock().unwrap().complete(ready.key);
            trace(format_args!(
                "frame={} gen={} release=queued reason=shutdown-before-present",
                ready.key.frame, ready.key.generation
            ));
        }
    }

    pub(in crate::platform) fn schedule_release_drain(&self) {
        schedule_release_drain(&self.release_scheduler);
    }

    pub(in crate::platform) fn register_backing(
        &self,
        descriptor: BackingDescriptor,
        port: mach_port_t,
    ) -> anyhow::Result<()> {
        let setup_started = Instant::now();
        let BackingDescriptor {
            id: backing,
            generation,
            width,
            height,
            pixel_format,
        } = descriptor;
        let import_started = Instant::now();
        anyhow::ensure!(
            pixel_format == core_video::pixel_buffer::kCVPixelFormatType_32BGRA,
            "unsupported IOSurface pixel format {pixel_format:#x}"
        );
        // The callback keeps the original Mach send right alive through this
        // lookup. IOSurfaceLookupFromMachPort retains the surface without
        // consuming the port, so the broker can receive its own right below.
        #[allow(deprecated)]
        let raw_surface = unsafe { io_surface::IOSurfaceLookupFromMachPort(port) };
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
        let import_duration = import_started.elapsed();
        let register_started = Instant::now();
        self.channel
            .register_backing(&self.channel_id, descriptor, port)?;
        let register_duration = register_started.elapsed();
        let mut backings = self.backings.lock().unwrap();
        // Engine replaced its pool, so older generations never return. Frames
        // already presented hold their own pixel buffer, and a late frame from
        // an older generation is rejected as out of order or missing.
        let mut released = 0;
        backings.retain(|&(old_backing, old_generation), _| {
            if old_generation >= generation {
                return true;
            }
            if let Err(error) =
                self.channel
                    .unregister_backing(&self.channel_id, old_backing, old_generation)
            {
                eprintln!("Photon presentation could not release backing: {error:#}");
            }
            released += 1;
            false
        });
        if released > 0 {
            trace(format_args!(
                "gen={generation} released {released} older backings"
            ));
        }
        backings.insert(
            (backing, generation),
            Backing {
                image,
                iosurface_id,
            },
        );
        trace(format_args!(
            "backing={backing} gen={generation} surface={iosurface_id} texture={width}x{height} setup surface_import={:.3}ms broker_register={:.3}ms total={:.3}ms",
            import_duration.as_secs_f64() * 1000.0,
            register_duration.as_secs_f64() * 1000.0,
            setup_started.elapsed().as_secs_f64() * 1000.0,
        ));
        Ok(())
    }

    pub(in crate::platform) fn receive_frame(
        &self,
        backing: u64,
        generation: u64,
        frame: u64,
        signal: u64,
        width: i32,
        height: i32,
    ) {
        self.performance.record_frame_handoff(generation, frame);
        let key = FrameKey {
            backing,
            generation,
            frame,
        };
        self.leases.lock().unwrap().receive(key);
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
        self.leases.lock().unwrap().accept(key);
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
            self.performance
                .record_frame_coalesced_before_install(replaced.key.generation, replaced.key.frame);
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

    pub(in crate::platform) fn take_surface(&self) -> Option<PresentedSurface> {
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
        let event = self.consumer_event.lock().unwrap().as_ref().cloned();
        if ready.signal != 0 && event.is_none() {
            self.leases.lock().unwrap().complete(ready.key);
            trace(format_args!(
                "frame={} gen={} release=queued reason=missing-consumer-event",
                ready.key.frame, ready.key.generation
            ));
            return None;
        }
        if std::env::var_os("PHOTON_VERBOSE").is_some()
            && let Some(event) = event.as_ref()
        {
            let producer_signaled = event.signaled_value();
            trace(format_args!(
                "frame={} gen={} producer-event signaled={} required={}",
                ready.key.frame, ready.key.generation, producer_signaled, ready.signal
            ));
        }
        let leases = self.leases.clone();
        let gpu_activity = self.gpu_activity.clone();
        let presented_leases = self.leases.clone();
        let navigation_id = self
            .navigation_timing
            .as_ref()
            .and_then(|profiler| profiler.navigation_id_for_first_surface());
        let key = ready.key;
        let image = backing.image.clone();
        let sampling = Arc::new(Sampling::default());
        let retired_until_submitted = Arc::new(Mutex::new(Vec::<RetiredSurface>::new()));
        let submitted_retirements = retired_until_submitted.clone();
        let submitted_sampling = sampling.clone();
        let completed_sampling = sampling.clone();
        let release_scheduler = self.release_scheduler.clone();
        let submitted_release_scheduler = self.release_scheduler.clone();
        let navigation_timing = self.navigation_timing.clone();
        let submitted_performance = self.performance.clone();
        let completed_performance = self.performance.clone();
        let presented_performance = self.performance.clone();
        let surface = ExternalMetalSurface::new(
            ExternalSurfaceDescriptor {
                identity: ExternalTextureIdentity {
                    resource_id: key.backing,
                    generation: key.generation,
                    iosurface_id: backing.iosurface_id,
                },
                // Keep the pooled IOSurface dimensions stable for texture
                // caching and layout, and pass the current frame's visible
                // content size separately for sampling.
                size: size(
                    gpui::DevicePixels(image.get_width() as i32),
                    gpui::DevicePixels(image.get_height() as i32),
                ),
                visible_size: size(
                    gpui::DevicePixels(ready.width.min(image.get_width() as i32)),
                    gpui::DevicePixels(ready.height.min(image.get_height() as i32)),
                ),
                pixel_format: image.get_pixel_format(),
            },
            image,
            if ready.signal == 0 {
                None
            } else {
                event.map(|event| MetalSharedEventWait {
                    event,
                    value: ready.signal,
                })
            },
            {
                let gpu_activity = gpu_activity.clone();
                let leases = leases.clone();
                // Command buffers on one queue are submitted in order, so once this
                // frame is submitted, no later command buffer samples the frames it
                // replaced. Each is released when its own last sampling completes,
                // without waiting for this frame's GPU work.
                move || {
                    presented_leases.lock().unwrap().present(key);
                    gpu_activity.submitted();
                    submitted_sampling.in_flight.fetch_add(1, Ordering::SeqCst);
                    let retired = std::mem::take(&mut *submitted_retirements.lock().unwrap());
                    let mut released_any = false;
                    for retired in retired {
                        retired.sampling.superseded.store(true, Ordering::SeqCst);
                        if retired.sampling.in_flight.load(Ordering::SeqCst) == 0 {
                            leases.lock().unwrap().complete(retired.key);
                            released_any = true;
                            trace(format_args!(
                                "frame={} gen={} release=queued superseded-by={}",
                                retired.key.frame, retired.key.generation, key.frame
                            ));
                        }
                    }
                    if released_any {
                        schedule_release_drain(&submitted_release_scheduler);
                    }
                }
            },
            move || {
                gpu_activity.completed();
                if let (Some(profiler), Some(navigation_id)) =
                    (navigation_timing.as_ref(), navigation_id)
                {
                    profiler.record_surface_gpu_complete(navigation_id);
                }
                trace(format_args!(
                    "frame={} gen={} command-buffer=completed",
                    key.frame, key.generation
                ));
                // The ledger ignores a second completion if the submit side also saw
                // this frame idle after superseding it.
                if completed_sampling.in_flight.fetch_sub(1, Ordering::SeqCst) == 1
                    && completed_sampling.superseded.load(Ordering::SeqCst)
                {
                    leases.lock().unwrap().complete(key);
                    trace(format_args!(
                        "frame={} gen={} release=queued after-last-sampling",
                        key.frame, key.generation
                    ));
                    schedule_release_drain(&release_scheduler);
                }
            },
        );
        let surface = if presented_performance.is_frame_timing_active() {
            let frame_timing_enabled = presented_performance.frame_timing_enabled_flag();
            let submitted_performance = submitted_performance.clone();
            let completed_performance = completed_performance.clone();
            surface.with_gpu_sampling_feedback(
                frame_timing_enabled,
                move |drawable_id| {
                    submitted_performance.record_gpu_submission(
                        key.generation,
                        key.frame,
                        drawable_id,
                    );
                },
                move |drawable_id| {
                    completed_performance.record_gpu_completion_callback(
                        key.generation,
                        key.frame,
                        drawable_id,
                    );
                },
            )
        } else {
            surface
        };
        let surface = if presented_performance.is_frame_timing_active() {
            let frame_timing_enabled = presented_performance.frame_timing_enabled_flag();
            surface.with_drawable_presentation_feedback(
                frame_timing_enabled,
                move |drawable_id, presented_time_seconds| {
                    presented_performance.record_drawable_presentation(
                        key.generation,
                        key.frame,
                        drawable_id,
                        presented_time_seconds,
                    );
                },
            )
        } else {
            surface
        };
        trace(format_args!(
            "frame={} gen={} surface-ready signal={}",
            key.frame, key.generation, ready.signal
        ));
        Some(PresentedSurface {
            surface,
            key,
            navigation_id,
            sampling,
            retired_until_submitted,
        })
    }

    pub(in crate::platform) fn has_pending_frame(&self) -> bool {
        self.latest.lock().unwrap().is_some()
    }
}

/// Uses the channel named by `photon run`, or creates a process-local channel
/// when macOS launches Photon from another app.
fn presentation_channel_id() -> String {
    static CHANNEL: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    CHANNEL
        .get_or_init(|| {
            std::env::var("PHOTON_PRESENTATION_CHANNEL_ID").unwrap_or_else(|_| {
                let started = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos();
                format!("photon-gpui-ce-{}-{started}", std::process::id())
            })
        })
        .clone()
}
