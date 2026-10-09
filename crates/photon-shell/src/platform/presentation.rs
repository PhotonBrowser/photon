//! IOSurface frame acceptance, presentation, and lease bookkeeping.
#![allow(deprecated)]

use core_video::pixel_buffer::CVPixelBuffer;
use gpui::{
    ExternalMetalSurface, ExternalSurfaceDescriptor, ExternalTextureIdentity, MetalSharedEventWait,
    size,
};
use io_surface::IOSurface;
use mach2::{port::mach_port_t, traps::mach_task_self};
use metal::SharedEvent;
use std::{
    collections::{HashMap, HashSet},
    ffi::c_void,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering},
    },
};

use super::{ffi::embedder, trace};
use photon_presentation_ipc::Channel;

pub(super) struct MachPortGuard(pub(super) mach_port_t);
impl Drop for MachPortGuard {
    fn drop(&mut self) {
        unsafe { mach2::mach_port::mach_port_deallocate(mach_task_self(), self.0) };
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct FrameKey {
    pub(super) backing: u64,
    pub(super) generation: u64,
    pub(super) frame: u64,
}

#[derive(Clone, Copy)]
pub(super) struct FrameReady {
    pub(super) key: FrameKey,
    pub(super) signal: u64,
    pub(super) width: i32,
    pub(super) height: i32,
}

#[derive(Clone, Copy)]
pub(super) struct Release {
    pub(super) key: FrameKey,
}

#[derive(Default)]
pub(super) struct LeaseLedger {
    received: HashSet<FrameKey>,
    accepted: HashSet<FrameKey>,
    presented: HashSet<FrameKey>,
    completed: HashSet<FrameKey>,
    released: HashSet<FrameKey>,
    pending: Vec<Release>,
}

impl LeaseLedger {
    pub(super) fn receive(&mut self, key: FrameKey) {
        assert!(
            self.received.insert(key),
            "duplicate native frame lease {key:?}"
        );
    }

    pub(super) fn accept(&mut self, key: FrameKey) {
        assert!(self.received.contains(&key));
        assert!(
            self.accepted.insert(key),
            "duplicate accepted frame {key:?}"
        );
    }

    pub(super) fn present(&mut self, key: FrameKey) {
        assert!(self.accepted.contains(&key));
        self.presented.insert(key);
    }

    pub(super) fn complete(&mut self, key: FrameKey) {
        if !self.released.contains(&key) && self.completed.insert(key) {
            self.pending.push(Release { key });
        }
    }

    pub(super) fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    pub(super) fn take_pending(&mut self) -> Vec<Release> {
        std::mem::take(&mut self.pending)
    }

    pub(super) fn release(&mut self, key: FrameKey) {
        assert!(
            self.completed.contains(&key),
            "released incomplete frame lease {key:?}"
        );
        assert!(
            self.released.insert(key),
            "duplicate native frame release {key:?}"
        );
    }

    pub(super) fn counts(&self) -> LeaseCounts {
        LeaseCounts {
            received: self.received.len(),
            accepted: self.accepted.len(),
            presented: self.presented.len(),
            completed: self.completed.len(),
            released: self.released.len(),
            outstanding: self.received.difference(&self.released).count(),
        }
    }
}

pub(super) struct LeaseCounts {
    pub(super) received: usize,
    pub(super) accepted: usize,
    pub(super) presented: usize,
    pub(super) completed: usize,
    pub(super) released: usize,
    pub(super) outstanding: usize,
}

#[derive(Default)]
pub(super) struct GpuActivity {
    progress: Mutex<GpuProgress>,
    idle: Condvar,
}

#[derive(Default)]
pub(super) struct GpuProgress {
    in_flight: usize,
    submitted: usize,
    completed: usize,
}

impl GpuActivity {
    pub(super) fn submitted(&self) {
        let mut progress = self.progress.lock().unwrap();
        progress.in_flight += 1;
        progress.submitted += 1;
    }

    pub(super) fn completed(&self) {
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

    pub(super) fn wait_until_idle(&self) {
        let mut progress = self.progress.lock().unwrap();
        while progress.in_flight != 0 {
            progress = self.idle.wait(progress).unwrap();
        }
    }

    pub(super) fn counts(&self) -> (usize, usize, usize) {
        let progress = self.progress.lock().unwrap();
        (progress.submitted, progress.completed, progress.in_flight)
    }
}

/// How a presented frame is being read by GPUI-CE command buffers.
#[derive(Default)]
pub(super) struct Sampling {
    in_flight: AtomicUsize,
    /// Set once a command buffer sampling a newer frame has been submitted, so
    /// no command buffer submitted afterwards samples this one.
    superseded: AtomicBool,
}

/// A frame no longer displayed, released to Engine once nothing samples it.
pub(super) struct RetiredSurface {
    pub(super) key: FrameKey,
    sampling: Arc<Sampling>,
}

pub(super) struct PresentedSurface {
    pub(super) surface: ExternalMetalSurface,
    pub(super) key: FrameKey,
    sampling: Arc<Sampling>,
    /// Frames this one replaced; they are superseded when it is first submitted.
    pub(super) retired_until_submitted: Arc<Mutex<Vec<RetiredSurface>>>,
}

impl PresentedSurface {
    pub(super) fn retire(&self) -> RetiredSurface {
        RetiredSurface {
            key: self.key,
            sampling: self.sampling.clone(),
        }
    }
}

pub(super) struct Backing {
    image: CVPixelBuffer,
    iosurface_id: u32,
}

fn schedule_release_drain(release_scheduler: &AtomicPtr<c_void>) {
    let runtime = release_scheduler.load(Ordering::Acquire);
    if !runtime.is_null() {
        unsafe { embedder::photon_runtime_schedule_native_release_drain(runtime) };
    }
}

pub(super) struct PresentationRuntime {
    channel: Channel,
    channel_id: String,
    consumer_event: Mutex<Option<SharedEvent>>,
    backings: Mutex<HashMap<(u64, u64), Backing>>,
    latest: Mutex<Option<FrameReady>>,
    latest_order: Mutex<(u64, u64)>,
    pub(super) leases: Arc<Mutex<LeaseLedger>>,
    pub(super) gpu_activity: Arc<GpuActivity>,
    pub(super) release_scheduler: Arc<AtomicPtr<c_void>>,
    accepting_frames: AtomicBool,
}

impl PresentationRuntime {
    pub(super) fn new(
        service: &str,
        channel_id: String,
        leases: Arc<Mutex<LeaseLedger>>,
        gpu_activity: Arc<GpuActivity>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            channel: Channel::connect(service)?,
            channel_id,
            consumer_event: Mutex::new(None),
            backings: Mutex::new(HashMap::new()),
            latest: Mutex::new(None),
            latest_order: Mutex::new((0, 0)),
            leases,
            gpu_activity,
            release_scheduler: Arc::new(AtomicPtr::new(std::ptr::null_mut())),
            accepting_frames: AtomicBool::new(true),
        })
    }

    pub(super) fn activate(&self) -> anyhow::Result<()> {
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

    pub(super) fn stop_accepting_frames(&self) {
        self.accepting_frames.store(false, Ordering::Release);
        if let Some(ready) = self.latest.lock().unwrap().take() {
            self.leases.lock().unwrap().complete(ready.key);
            trace(format_args!(
                "frame={} gen={} release=queued reason=shutdown-before-present",
                ready.key.frame, ready.key.generation
            ));
        }
    }

    pub(super) fn schedule_release_drain(&self) {
        schedule_release_drain(&self.release_scheduler);
    }

    pub(super) fn register_backing(
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
        Ok(())
    }

    pub(super) fn receive_frame(
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

    pub(super) fn take_surface(&self) -> Option<PresentedSurface> {
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
        let key = ready.key;
        let image = backing.image.clone();
        let sampling = Arc::new(Sampling::default());
        let retired_until_submitted = Arc::new(Mutex::new(Vec::<RetiredSurface>::new()));
        let submitted_retirements = retired_until_submitted.clone();
        let submitted_sampling = sampling.clone();
        let completed_sampling = sampling.clone();
        let release_scheduler = self.release_scheduler.clone();
        let submitted_release_scheduler = self.release_scheduler.clone();
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
        trace(format_args!(
            "frame={} gen={} surface-ready signal={}",
            key.frame, key.generation, ready.signal
        ));
        Some(PresentedSurface {
            surface,
            key,
            sampling,
            retired_until_submitted,
        })
    }
}
