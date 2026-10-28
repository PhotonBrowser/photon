//! IOSurface frame acceptance, presentation, and lease bookkeeping.
#![allow(deprecated)]

use anyhow::Context as _;
use io_surface::IOSurface;
use mach2::{port::mach_port_t, traps::mach_task_self};
use metal::SharedEvent;
use photon_performance::PerformanceMonitor;
use std::{
    collections::HashMap,
    ffi::c_void,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicPtr, Ordering},
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
    pub(in crate::platform) consumer_gpu_work_complete: bool,
}

/// Where a frame lease the Engine handed over has got to.
#[derive(Default)]
struct Lease {
    accepted: bool,
    completed: bool,
    /// Core Animation displayed it, so the window server may still read it.
    presented: bool,
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
        let lease = self.held.get_mut(&key).expect("presented an unknown frame");
        assert!(lease.accepted);
        if !lease.presented {
            lease.presented = true;
            self.counts.presented += 1;
        }
    }

    /// Queues a held lease's release, once. A frame never displayed can be
    /// reused at once. A frame Core Animation displayed is not reported
    /// finished while the window server may still read it: only Engine's
    /// compositor can see when the window server stops reading its IOSurface,
    /// and it adds a store rather than wait.
    pub(in crate::platform) fn complete(&mut self, key: FrameKey) {
        if let Some(lease) = self.held.get_mut(&key)
            && !lease.completed
        {
            lease.completed = true;
            self.counts.completed += 1;
            self.pending.push(Release {
                key,
                consumer_gpu_work_complete: !lease.presented,
            });
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

/// A frame shown, or about to be shown, in the page's Core Animation layer.
pub(in crate::platform) struct PresentedSurface {
    pub(in crate::platform) iosurface: IOSurface,
    pub(in crate::platform) visible_width: i32,
    pub(in crate::platform) visible_height: i32,
    pub(in crate::platform) key: FrameKey,
    pub(in crate::platform) navigation_id: Option<u64>,
    /// Frames this one replaced, released once it is displayed.
    pub(in crate::platform) replaced: Vec<FrameKey>,
}

impl PresentedSurface {
    pub(in crate::platform) fn timing_identity(&self) -> (u64, u64) {
        (self.key.generation, self.key.frame)
    }

    /// Every frame lease this one holds, to release when the page closes.
    pub(in crate::platform) fn into_release_keys(mut self) -> Vec<FrameKey> {
        self.replaced.push(self.key);
        self.replaced
    }
}

/// An Engine backing store, held as its IOSurface alone. A CoreVideo buffer
/// would mark the surface in use for as long as it lived, and Engine's
/// compositor reads that mark to tell when the window server has finished
/// with a surface it displayed.
pub(in crate::platform) struct Backing {
    iosurface: IOSurface,
    width: i32,
    height: i32,
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
    pub(in crate::platform) release_scheduler: Arc<AtomicPtr<c_void>>,
    navigation_timing: Option<Arc<NavigationTimingProfiler>>,
    performance: Arc<PerformanceMonitor>,
    accepting_frames: AtomicBool,
}

impl PresentationRuntime {
    pub(in crate::platform) fn new(
        leases: Arc<Mutex<LeaseLedger>>,
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
        let import_duration = import_started.elapsed();
        let register_started = Instant::now();
        self.channel
            .register_backing(&self.channel_id, descriptor, port)?;
        let register_duration = register_started.elapsed();
        let mut backings = self.backings.lock().unwrap();
        // Engine replaced its pool, so older generations never return. Frames
        // already presented hold their own IOSurface, and a late frame from
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
                iosurface,
                width: width as i32,
                height: height as i32,
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

    /// Whether the producer has finished writing a frame. Core Animation
    /// cannot wait for the producer's GPU work, so a frame is shown only once
    /// that work is done.
    fn producer_finished(&self, ready: &FrameReady) -> bool {
        if ready.signal == 0 {
            return true;
        }
        // `take_surface` releases a signaled frame without a consumer event.
        self.consumer_event
            .lock()
            .unwrap()
            .as_ref()
            .is_none_or(|event| event.signaled_value() >= ready.signal)
    }

    /// Calls `on_finished` on the main thread once the newest frame's
    /// producer work completes, and returns true; returns false when there is
    /// no such wait because it is already finished or there is no frame.
    pub(in crate::platform) fn notify_when_latest_finished(
        &self,
        on_finished: impl Fn() + 'static,
    ) -> bool {
        let Some(ready) = *self.latest.lock().unwrap() else {
            return false;
        };
        if self.producer_finished(&ready) {
            return false;
        }
        let Some(event) = self.consumer_event.lock().unwrap().clone() else {
            return false;
        };
        notify_on_main_thread(&event, ready.signal, on_finished);
        true
    }

    pub(in crate::platform) fn take_surface(&self) -> Option<PresentedSurface> {
        let ready = {
            let mut latest = self.latest.lock().unwrap();
            if !self.producer_finished(latest.as_ref()?) {
                return None;
            }
            latest.take()?
        };
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
        if ready.signal != 0 && self.consumer_event.lock().unwrap().is_none() {
            self.leases.lock().unwrap().complete(ready.key);
            trace(format_args!(
                "frame={} gen={} release=queued reason=missing-consumer-event",
                ready.key.frame, ready.key.generation
            ));
            return None;
        }
        let navigation_id = self
            .navigation_timing
            .as_ref()
            .and_then(|profiler| profiler.navigation_id_for_first_surface());
        trace(format_args!(
            "frame={} gen={} surface-ready signal={}",
            ready.key.frame, ready.key.generation, ready.signal
        ));
        Some(PresentedSurface {
            iosurface: backing.iosurface.clone(),
            visible_width: ready.width.min(backing.width),
            visible_height: ready.height.min(backing.height),
            key: ready.key,
            navigation_id,
            replaced: Vec::new(),
        })
    }

    /// Whether a frame is ready to install now.
    pub(in crate::platform) fn has_pending_frame(&self) -> bool {
        self.latest
            .lock()
            .unwrap()
            .is_some_and(|ready| self.producer_finished(&ready))
    }

    /// Records that Core Animation now shows `presented`, and releases the
    /// frames it replaced. Like Ladybird's view, the previous front buffer goes
    /// back as soon as the next one is shown.
    pub(in crate::platform) fn present(&self, presented: &mut PresentedSurface) {
        let mut leases = self.leases.lock().unwrap();
        leases.present(presented.key);
        if presented.replaced.is_empty() {
            return;
        }
        for key in presented.replaced.drain(..) {
            leases.complete(key);
            trace(format_args!(
                "frame={} gen={} release=queued superseded-by={}",
                key.frame, key.generation, presented.key.frame
            ));
        }
        drop(leases);
        schedule_release_drain(&self.release_scheduler);
    }
}

/// The block signature `-[MTLSharedEvent notifyListener:atValue:block:]` requires.
struct SharedEventNotificationEncoding;

// SAFETY: MTLSharedEventNotificationBlock is `void (^)(id<MTLSharedEvent>, uint64_t)`.
unsafe impl block2::ManualBlockEncoding for SharedEventNotificationEncoding {
    type Arguments = (*mut objc2::runtime::AnyObject, u64);
    type Return = ();
    const ENCODING_CSTR: &'static std::ffi::CStr = c"v24@?0@8Q16";
}

/// Runs `on_signaled` on the main queue once `event` reaches `value`, without
/// a thread waiting for it.
fn notify_on_main_thread(event: &SharedEvent, value: u64, on_signaled: impl Fn() + 'static) {
    use metal::foreign_types::ForeignType as _;
    use objc2::{class, msg_send, rc::Retained, runtime::AnyObject};

    thread_local! {
        static MAIN_QUEUE_LISTENER: Retained<AnyObject> = {
            let queue = dispatch2::DispatchQueue::main();
            // SAFETY: MTLSharedEventListener's designated initializer takes a
            // dispatch queue; the main queue lives for the process.
            unsafe {
                let listener: *mut AnyObject = msg_send![class!(MTLSharedEventListener), alloc];
                let listener: *mut AnyObject = msg_send![listener, initWithDispatchQueue: queue];
                Retained::from_raw(listener).expect("could not create a Metal event listener")
            }
        };
    }
    let block = block2::RcBlock::with_encoding::<_, _, _, SharedEventNotificationEncoding>(
        move |_: *mut AnyObject, _: u64| on_signaled(),
    );
    MAIN_QUEUE_LISTENER.with(|listener| {
        // SAFETY: Metal copies the block and calls it once on the listener's
        // main queue, the thread that created it.
        unsafe {
            let event = event.as_ptr().cast::<AnyObject>();
            let _: () =
                msg_send![event, notifyListener: &**listener, atValue: value, block: &*block];
        }
    });
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
