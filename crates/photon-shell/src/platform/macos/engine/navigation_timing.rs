//! Opt-in navigation milestones collected at the Photon embedder boundary.

use std::{
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

const NAVIGATION_STARTED: i32 = 0;
const DOCUMENT_COMMITTED: i32 = 1;
const LOAD_FINISHED: i32 = 2;
const FIRST_FRAME_READY: i32 = 3;

pub(in crate::platform::macos) fn navigation_profile_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("PHOTON_NAVIGATION_PROFILE").is_some())
}

#[derive(Default)]
pub(in crate::platform::macos) struct NavigationTimingProfiler {
    next_id: AtomicU64,
    current: Mutex<Option<NavigationTimeline>>,
}

#[derive(Default)]
struct NavigationTimeline {
    id: u64,
    url: String,
    started: Option<Instant>,
    committed: Option<Instant>,
    load_finished: Option<Instant>,
    first_frame_ready: Option<Instant>,
    surface_installed: Option<Instant>,
    surface_rendered: Option<Instant>,
    surface_gpu_complete: Option<Instant>,
}

impl NavigationTimingProfiler {
    pub(in crate::platform::macos) fn record_event(&self, event: i32, url: &str) {
        let now = Instant::now();
        let mut current = self.current.lock().unwrap();
        match event {
            NAVIGATION_STARTED => {
                let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
                let timeline = NavigationTimeline {
                    id,
                    url: origin(url),
                    started: Some(now),
                    ..NavigationTimeline::default()
                };
                eprintln!(
                    "[PhotonNavigationTiming] nav={id} event=start url={}",
                    timeline.url
                );
                *current = Some(timeline);
            }
            DOCUMENT_COMMITTED => {
                let timeline = current.get_or_insert_with(|| self.new_unstarted_timeline(url));
                if timeline.surface_rendered.is_some() {
                    *timeline = self.new_unstarted_timeline(url);
                }
                timeline.url = origin(url);
                timeline.committed = Some(now);
                self.log_event(timeline, "document_committed", now);
            }
            LOAD_FINISHED => {
                if let Some(timeline) = current.as_mut() {
                    timeline.url = origin(url);
                    timeline.load_finished = Some(now);
                    self.log_event(timeline, "load_finished", now);
                }
            }
            FIRST_FRAME_READY => {
                if let Some(timeline) = current.as_mut()
                    && timeline.committed.is_some()
                    && timeline.first_frame_ready.is_none()
                {
                    timeline.first_frame_ready = Some(now);
                    self.log_event(timeline, "first_engine_frame_ready", now);
                }
            }
            _ => {}
        }
    }

    pub(in crate::platform::macos) fn navigation_id_for_first_surface(&self) -> Option<u64> {
        let current = self.current.lock().unwrap();
        let timeline = current.as_ref()?;
        (timeline.committed.is_some()
            && timeline.first_frame_ready.is_some()
            && timeline.surface_installed.is_none())
        .then_some(timeline.id)
    }

    pub(in crate::platform::macos) fn record_surface_installed(&self, navigation_id: u64) {
        let now = Instant::now();
        let mut current = self.current.lock().unwrap();
        let Some(timeline) = current.as_mut() else {
            return;
        };
        if timeline.id != navigation_id
            || timeline.first_frame_ready.is_none()
            || timeline.surface_installed.is_some()
        {
            return;
        }
        timeline.surface_installed = Some(now);
        self.log_event(timeline, "surface_installed", now);
    }

    pub(in crate::platform::macos) fn record_surface_rendered(&self, navigation_id: u64) {
        let now = Instant::now();
        let mut current = self.current.lock().unwrap();
        let Some(timeline) = current.as_mut() else {
            return;
        };
        if timeline.id != navigation_id
            || timeline.surface_installed.is_none()
            || timeline.surface_rendered.is_some()
        {
            return;
        }
        timeline.surface_rendered = Some(now);
        self.log_event(timeline, "first_gpui_render", now);
    }

    pub(in crate::platform::macos) fn record_surface_gpu_complete(&self, navigation_id: u64) {
        let now = Instant::now();
        let mut current = self.current.lock().unwrap();
        let Some(timeline) = current.as_mut() else {
            return;
        };
        if timeline.id != navigation_id
            || timeline.surface_rendered.is_none()
            || timeline.surface_gpu_complete.is_some()
        {
            return;
        }
        timeline.surface_gpu_complete = Some(now);
        self.log_event(timeline, "first_page_gpu_present_complete", now);
        eprintln!(
            "[PhotonNavigationTiming] nav={} event=summary url={} start_to_commit_ms={} commit_to_engine_frame_ms={} engine_frame_to_surface_ms={} surface_to_gpui_render_ms={} gpui_render_to_gpu_present_complete_ms={} start_to_gpu_present_complete_ms={} load_finished_ms={}",
            timeline.id,
            timeline.url,
            elapsed_ms(timeline.started, timeline.committed),
            elapsed_ms(timeline.committed, timeline.first_frame_ready),
            elapsed_ms(timeline.first_frame_ready, timeline.surface_installed),
            elapsed_ms(timeline.surface_installed, timeline.surface_rendered),
            elapsed_ms(timeline.surface_rendered, timeline.surface_gpu_complete),
            elapsed_ms(timeline.started, timeline.surface_gpu_complete),
            elapsed_ms(timeline.started, timeline.load_finished),
        );
    }

    fn new_unstarted_timeline(&self, url: &str) -> NavigationTimeline {
        NavigationTimeline {
            id: self.next_id.fetch_add(1, Ordering::Relaxed) + 1,
            url: origin(url),
            ..NavigationTimeline::default()
        }
    }

    fn log_event(&self, timeline: &NavigationTimeline, event: &str, at: Instant) {
        eprintln!(
            "[PhotonNavigationTiming] nav={} event={} url={} since_start_ms={}",
            timeline.id,
            event,
            timeline.url,
            elapsed_ms(timeline.started, Some(at)),
        );
    }
}

fn elapsed_ms(start: Option<Instant>, end: Option<Instant>) -> String {
    match (start, end) {
        (Some(start), Some(end)) => {
            format!("{:.3}", end.duration_since(start).as_secs_f64() * 1000.0)
        }
        _ => "unavailable".to_owned(),
    }
}

fn origin(url: &str) -> String {
    let Some((scheme, remainder)) = url.split_once("://") else {
        return url.split(['?', '#']).next().unwrap_or(url).to_owned();
    };
    let authority = remainder
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .rsplit('@')
        .next()
        .unwrap_or_default();
    format!("{scheme}://{authority}")
}
