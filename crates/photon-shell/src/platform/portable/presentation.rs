//! Portable page-frame handoff used where Engine surfaces cannot be imported.
//!
//! The Ladybird compositor currently publishes a CPU fallback frame outside
//! macOS. This adapter owns the pixel copy and alpha conversion and wraps it as
//! an opaque GPUI image; shared browser views only see `PresentedSurface`.

use gpui::RenderImage;
use photon_performance::PerformanceMonitor;
use std::{
    ffi::c_void,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering},
    },
    time::Instant,
};

use crate::platform::engine::navigation_timing::NavigationTimingProfiler;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::platform) struct FrameKey {
    pub(in crate::platform) backing: u64,
    pub(in crate::platform) generation: u64,
    pub(in crate::platform) frame: u64,
}

#[derive(Clone, Copy)]
pub(in crate::platform) struct Release {
    pub(in crate::platform) key: FrameKey,
}

#[derive(Default)]
pub(in crate::platform) struct LeaseLedger;

impl LeaseLedger {
    pub(in crate::platform) fn complete(&mut self, _: FrameKey) {}

    pub(in crate::platform) fn has_pending(&self) -> bool {
        false
    }

    pub(in crate::platform) fn take_pending(&mut self) -> Vec<Release> {
        Vec::new()
    }

    pub(in crate::platform) fn release(&mut self, _: FrameKey) {}

    pub(in crate::platform) fn counts(&self) -> LeaseCounts {
        LeaseCounts::default()
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

pub(in crate::platform) struct PresentedSurface {
    pub(in crate::platform) image: Arc<RenderImage>,
    pub(in crate::platform) key: FrameKey,
    pub(in crate::platform) navigation_id: Option<u64>,
}

impl PresentedSurface {
    pub(in crate::platform) fn timing_identity(&self) -> (u64, u64) {
        (self.key.generation, self.key.frame)
    }

    /// Every frame lease this one holds, to release when the page closes.
    pub(in crate::platform) fn into_release_keys(self) -> Vec<FrameKey> {
        vec![self.key]
    }
}

pub(in crate::platform) struct PresentationRuntime {
    latest: Mutex<Option<PresentedSurface>>,
    next_frame: AtomicU64,
    pub(in crate::platform) leases: Arc<Mutex<LeaseLedger>>,
    pub(in crate::platform) release_scheduler: Arc<AtomicPtr<c_void>>,
    navigation_timing: Option<Arc<NavigationTimingProfiler>>,
    _performance: Arc<PerformanceMonitor>,
    accepting_frames: AtomicBool,
}

impl PresentationRuntime {
    pub(in crate::platform) fn new(
        leases: Arc<Mutex<LeaseLedger>>,
        navigation_timing: Option<Arc<NavigationTimingProfiler>>,
        performance: Arc<PerformanceMonitor>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            latest: Mutex::new(None),
            next_frame: AtomicU64::new(0),
            leases,
            release_scheduler: Arc::new(AtomicPtr::new(std::ptr::null_mut())),
            navigation_timing,
            _performance: performance,
            accepting_frames: AtomicBool::new(true),
        })
    }

    pub(in crate::platform) fn activate(&self) -> anyhow::Result<()> {
        Ok(())
    }

    pub(in crate::platform) fn stop_accepting_frames(&self) {
        self.accepting_frames.store(false, Ordering::Release);
        self.latest.lock().unwrap().take();
    }

    /// Copies a transient premultiplied BGRA frame from the C ABI and converts
    /// it to GPUI's straight-alpha BGRA `RenderImage` format.
    pub(in crate::platform) unsafe fn publish_cpu_frame(
        &self,
        width: i32,
        height: i32,
        stride: usize,
        pixels: *const u8,
        length: usize,
    ) -> Option<(u64, u64, u64, u64)> {
        if !self.accepting_frames.load(Ordering::Acquire) || width <= 0 || height <= 0 {
            return None;
        }
        let width = width as usize;
        let height = height as usize;
        let row_bytes = width.checked_mul(4)?;
        if stride < row_bytes || pixels.is_null() {
            return None;
        }
        let required = stride.checked_mul(height)?;
        if length < required {
            return None;
        }

        // The Engine owns this memory only for the callback's duration.
        let source = unsafe { std::slice::from_raw_parts(pixels, required) };
        let allocation_started = Instant::now();
        let mut pixels = vec![0; row_bytes.checked_mul(height)?];
        let allocation_microseconds =
            u64::try_from(allocation_started.elapsed().as_micros()).unwrap_or(u64::MAX);
        let copy_started = Instant::now();
        for y in 0..height {
            let source_row = &source[y * stride..y * stride + row_bytes];
            let target_row = &mut pixels[y * row_bytes..(y + 1) * row_bytes];
            for pixel in 0..width {
                let offset = pixel * 4;
                let alpha = u16::from(source_row[offset + 3]);
                target_row[offset + 3] = u8::try_from(alpha).expect("alpha is an 8-bit channel");
                for channel in 0..3 {
                    let straight = if alpha == 0 {
                        0
                    } else {
                        (u16::from(source_row[offset + channel]) * u16::from(u8::MAX) + alpha / 2)
                            .checked_div(alpha)
                            .expect("alpha is nonzero")
                    }
                    .min(u16::from(u8::MAX));
                    target_row[offset + channel] =
                        u8::try_from(straight).expect("channel value is clamped to 8 bits");
                }
            }
        }
        let copy_microseconds =
            u64::try_from(copy_started.elapsed().as_micros()).unwrap_or(u64::MAX);
        let image = image::RgbaImage::from_raw(
            u32::try_from(width).ok()?,
            u32::try_from(height).ok()?,
            pixels,
        )?;
        let image = Arc::new(RenderImage::new([image::Frame::new(image)]));
        let frame = self.next_frame.fetch_add(1, Ordering::Relaxed) + 1;
        let navigation_id = self
            .navigation_timing
            .as_ref()
            .and_then(|timing| timing.navigation_id_for_first_surface());
        let presented = PresentedSurface {
            image,
            key: FrameKey {
                backing: 0,
                generation: 1,
                frame,
            },
            navigation_id,
        };
        let mut latest = self.latest.lock().unwrap();
        if latest
            .as_ref()
            .is_none_or(|current| current.key.frame < frame)
        {
            *latest = Some(presented);
        }
        Some((1, frame, allocation_microseconds, copy_microseconds))
    }

    /// CPU frames are complete when published, so there is never a wait.
    pub(in crate::platform) fn notify_when_latest_finished(&self, _: impl Fn() + 'static) -> bool {
        false
    }

    pub(in crate::platform) fn take_surface(&self) -> Option<PresentedSurface> {
        self.latest.lock().unwrap().take()
    }

    pub(in crate::platform) fn has_pending_frame(&self) -> bool {
        self.latest.lock().unwrap().is_some()
    }
}
