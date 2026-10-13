//! A live view of Photon's shell and tab-owned Engine resource usage.

use gpui::{Context, FontWeight, Render, Role, Task, Window, div, prelude::*, px, rgb, rgba};
use photon_performance::{
    ChartPoint, PerformanceHistory, PerformanceTask, ProcessDiagnostics, ProcessSampler,
    format_bytes, format_rate, performance_chart,
};
use std::{collections::VecDeque, time::Instant};

use super::super::super::engine::PerformanceDashboardLease;
use super::super::layout::{h_stack, v_stack};
use super::super::settings::Settings;
use super::super::{metrics, theme::ThemeColors, theme::palette};
use super::layout::{group, heading, secondary_text};
use super::{PageContext, PageDefinition, PageIcon};

const SAMPLE_INTERVAL_MS: u64 = 500;
const EVENT_LOOP_PROBE_INTERVAL_MS: u64 = 100;
const EVENT_LOOP_HISTORY_SECONDS: u64 = 60;

/// The native performance and task page.
pub(super) const PAGE: PageDefinition = PageDefinition {
    name: "performance",
    title: "Performance",
    icon: PageIcon::Logo,
    shows_address: true,
    single_tab: true,
    build: |context, cx| cx.new(|cx| PerformancePage::new(context, cx)).into(),
};

struct EventLoopDelay {
    recorded_at: Instant,
    delay_ms: f64,
}

pub(in super::super) struct PerformancePage {
    context: PageContext,
    _monitor: PerformanceDashboardLease,
    _sampler: ProcessSampler,
    shell: ProcessDiagnostics,
    tasks: Vec<PerformanceTask>,
    history: PerformanceHistory,
    delays: VecDeque<EventLoopDelay>,
    _sample_task: Task<()>,
    _event_loop_task: Task<()>,
}

impl PerformancePage {
    fn new(context: PageContext, cx: &mut Context<Self>) -> Self {
        let runtime = context.runtime.clone();
        let mut page = Self {
            context,
            _monitor: runtime.acquire_performance_dashboard(),
            _sampler: ProcessSampler::default(),
            shell: ProcessDiagnostics::default(),
            tasks: Vec::new(),
            history: PerformanceHistory::default(),
            delays: VecDeque::new(),
            _sample_task: cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(SAMPLE_INTERVAL_MS))
                        .await;
                    if this
                        .update(cx, |page, cx| {
                            page.sample();
                            cx.notify();
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            }),
            _event_loop_task: cx.spawn(async move |this, cx| {
                let interval = std::time::Duration::from_millis(EVENT_LOOP_PROBE_INTERVAL_MS);
                loop {
                    let deadline = Instant::now() + interval;
                    cx.background_executor().timer(interval).await;
                    if this
                        .update(cx, |page, cx| {
                            page.record_event_loop_delay(deadline, Instant::now());
                            if let Some(delay) = page.delays.back()
                                && delay.recorded_at >= deadline
                            {
                                cx.notify();
                            }
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            }),
        };
        page.sample();
        page
    }

    fn sample(&mut self) {
        self.shell = self._sampler.sample();
        self.tasks = self.context.runtime.performance_tasks();
        self.tasks.sort_by(|left, right| {
            right
                .diagnostics
                .cpu_percent
                .unwrap_or(0.0)
                .total_cmp(&left.diagnostics.cpu_percent.unwrap_or(0.0))
                .then_with(|| {
                    right
                        .diagnostics
                        .memory_bytes
                        .unwrap_or(0)
                        .cmp(&left.diagnostics.memory_bytes.unwrap_or(0))
                })
        });

        self.history
            .push(self.known_cpu_percent(), self.known_memory_bytes());
        self.expire_old_delays(Instant::now());
    }

    fn known_cpu_percent(&self) -> Option<f64> {
        let mut total = self.shell.cpu_percent?;
        for task in &self.tasks {
            total += task.diagnostics.cpu_percent?;
        }
        Some(total)
    }

    fn known_memory_bytes(&self) -> Option<u64> {
        let mut total = self.shell.memory_bytes?;
        for task in &self.tasks {
            total = total.checked_add(task.diagnostics.memory_bytes?)?;
        }
        Some(total)
    }

    fn record_event_loop_delay(&mut self, deadline: Instant, observed_at: Instant) {
        let delay = observed_at.saturating_duration_since(deadline);
        if delay.as_millis() >= u128::from(metrics::PERFORMANCE_EVENT_LOOP_DELAY_THRESHOLD_MS) {
            self.delays.push_back(EventLoopDelay {
                recorded_at: observed_at,
                delay_ms: delay.as_secs_f64() * 1000.0,
            });
        }
        self.expire_old_delays(observed_at);
    }

    fn expire_old_delays(&mut self, now: Instant) {
        while self.delays.front().is_some_and(|delay| {
            now.duration_since(delay.recorded_at).as_secs() > EVENT_LOOP_HISTORY_SECONDS
        }) {
            self.delays.pop_front();
        }
    }

    fn page_header(&self, palette: ThemeColors) -> impl IntoElement {
        heading(
            "performance-title",
            "Performance",
            "Live resource use across Photon and its open tabs.",
            palette,
        )
    }

    fn summary(&self, palette: ThemeColors) -> impl IntoElement {
        h_stack()
            .w_full()
            .gap(px(metrics::PERFORMANCE_PAGE_GAP))
            .child(summary_card(
                "CPU",
                self.known_cpu_percent()
                    .map_or_else(|| "—".into(), |cpu| format!("{cpu:.1}%")),
                "logical CPU use; may exceed 100%",
                palette,
            ))
            .child(summary_card(
                "Memory",
                self.known_memory_bytes()
                    .map_or_else(|| "—".into(), format_bytes),
                "shell plus tab-owned processes",
                palette,
            ))
            .child(summary_card(
                "Tasks",
                (self.tasks.len() + 1).to_string(),
                "Photon shell and open tab views",
                palette,
            ))
    }

    fn charts(&self, palette: ThemeColors) -> impl IntoElement {
        h_stack()
            .w_full()
            .gap(px(metrics::PERFORMANCE_PAGE_GAP))
            .child(chart_card(
                "CPU history",
                "Photon CPU",
                self.known_cpu_percent()
                    .map_or_else(|| "Waiting for samples".into(), |cpu| format!("{cpu:.1}%")),
                self.history
                    .samples()
                    .into_iter()
                    .map(|sample| ChartPoint {
                        elapsed_seconds: sample.elapsed_seconds,
                        value: sample.cpu_percent,
                    })
                    .collect(),
                palette,
            ))
            .child(chart_card(
                "Memory history",
                "Photon memory",
                self.known_memory_bytes()
                    .map_or_else(|| "Waiting for samples".into(), format_bytes),
                self.history
                    .samples()
                    .into_iter()
                    .map(|sample| ChartPoint {
                        elapsed_seconds: sample.elapsed_seconds,
                        value: sample.memory_bytes.map(|bytes| bytes as f64 / 1_000_000.0),
                    })
                    .collect(),
                palette,
            ))
    }

    fn task_list(&self, palette: ThemeColors) -> impl IntoElement {
        let rows = std::iter::once(shell_task(self.shell))
            .chain(self.tasks.iter().cloned())
            .map(|task| task_row(task, palette));
        v_stack()
            .w_full()
            .gap(px(metrics::MENU_ITEM_GAP))
            .child(section_heading("Tasks", palette))
            .child(
                group(palette)
                    .w_full()
                    .gap(px(metrics::PERFORMANCE_TASK_GAP))
                    .children(rows),
            )
            .child(secondary_text(
                "Tab rows combine WebContent and workers owned by that tab. Shared Engine services and GPU allocations are not included in these totals.",
                palette,
            ))
            .child(secondary_text(
                "Memory uses the operating system's physical-footprint or private-resident estimate. Managed heap is a separate, last-GC estimate.",
                palette,
            ))
    }

    fn event_loop_section(&self, palette: ThemeColors) -> impl IntoElement {
        let delays = self.delays.iter().rev().take(5).map(|delay| {
            let age = Instant::now()
                .duration_since(delay.recorded_at)
                .as_secs_f64();
            h_stack()
                .w_full()
                .justify_between()
                .child(format!("{:.0} ms", delay.delay_ms))
                .child(secondary_text(format!("{age:.1}s ago"), palette))
        });
        let details = if self.delays.is_empty() {
            v_stack()
                .child(format!(
                    "No delays over {} ms in the last minute",
                    metrics::PERFORMANCE_EVENT_LOOP_DELAY_THRESHOLD_MS
                ))
                .child(secondary_text(
                    "Photon's UI loop is checked every 100 ms while this page is open.",
                    palette,
                ))
        } else {
            v_stack()
                .gap(px(metrics::MENU_ITEM_GAP))
                .child(format!(
                    "{} delays over {} ms in the last minute",
                    self.delays.len(),
                    metrics::PERFORMANCE_EVENT_LOOP_DELAY_THRESHOLD_MS
                ))
                .children(delays)
        };

        v_stack()
            .w_full()
            .gap(px(metrics::MENU_ITEM_GAP))
            .child(section_heading("Main-thread responsiveness", palette))
            .child(
                group(palette)
                    .w_full()
                    .gap(px(metrics::PERFORMANCE_TASK_GAP))
                    .child(details),
            )
            .child(secondary_text(
                "These are delays returning to Photon's UI loop. They show when the shell could not run its next update; they do not identify the code that caused the delay.",
                palette,
            ))
    }
}

impl Render for PerformancePage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = palette(window, cx);
        v_stack()
            .id("performance-page")
            .size_full()
            .overflow_y_scroll()
            .elastic_scroll(
                crate::platform::supports_elastic_scroll() && Settings::get(cx).elastic_scroll,
            )
            .p(px(metrics::PERFORMANCE_PAGE_INSET))
            .items_center()
            .child(
                v_stack()
                    .w_full()
                    .max_w(px(metrics::PERFORMANCE_PAGE_MAX_WIDTH))
                    .gap(px(metrics::PERFORMANCE_PAGE_SECTION_GAP))
                    .child(self.page_header(palette))
                    .child(self.summary(palette))
                    .child(self.charts(palette))
                    .child(self.task_list(palette))
                    .child(self.event_loop_section(palette)),
            )
    }
}

fn summary_card(
    label: &'static str,
    value: String,
    detail: impl Into<gpui::SharedString>,
    palette: ThemeColors,
) -> impl IntoElement {
    v_stack()
        .flex_1()
        .min_w(px(0.0))
        .p(px(metrics::PERFORMANCE_CARD_PADDING))
        .gap(px(metrics::MENU_ITEM_GAP))
        .rounded(px(metrics::SURFACE_RADIUS))
        .border_1()
        .border_color(rgba(palette.menu_border))
        .bg(rgba(palette.menu_surface))
        .child(secondary_text(label, palette))
        .child(
            div()
                .text_size(px(metrics::INTERNAL_PAGE_SECTION_SIZE))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(palette.text_primary))
                .child(value),
        )
        .child(secondary_text(detail, palette))
}

fn chart_card(
    id: &'static str,
    title: &'static str,
    value: String,
    points: Vec<ChartPoint>,
    palette: ThemeColors,
) -> impl IntoElement {
    v_stack()
        .flex_1()
        .min_w(px(0.0))
        .p(px(metrics::PERFORMANCE_CARD_PADDING))
        .gap(px(metrics::PERFORMANCE_TASK_GAP))
        .rounded(px(metrics::SURFACE_RADIUS))
        .border_1()
        .border_color(rgba(palette.menu_border))
        .bg(rgba(palette.menu_surface))
        .child(
            h_stack()
                .w_full()
                .items_center()
                .justify_between()
                .child(section_heading(title, palette))
                .child(
                    div()
                        .text_size(px(metrics::INTERNAL_PAGE_BODY_SIZE))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(palette.text_primary))
                        .child(value),
                ),
        )
        .child(
            div()
                .id(id)
                .aria_label(format!("{title}, last 60 seconds"))
                .w_full()
                .h(px(metrics::PERFORMANCE_CHART_HEIGHT))
                .child(performance_chart(
                    points,
                    palette.chosen,
                    palette.menu_border,
                )),
        )
        .child(secondary_text("Last 60 seconds", palette))
}

fn section_heading(title: &'static str, palette: ThemeColors) -> impl IntoElement {
    div()
        .id(title)
        .role(Role::Heading)
        .aria_level(2)
        .text_size(px(metrics::INTERNAL_PAGE_BODY_SIZE))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(palette.text_primary))
        .child(title)
}

fn shell_task(diagnostics: ProcessDiagnostics) -> PerformanceTask {
    PerformanceTask {
        id: 0,
        title: "Photon shell".into(),
        url: "Application process".into(),
        loading: false,
        diagnostics: photon_performance::PerformanceDiagnostics {
            cpu_percent: diagnostics.cpu_percent,
            memory_bytes: diagnostics.memory_bytes,
            ..photon_performance::PerformanceDiagnostics::default()
        },
    }
}

fn task_row(task: PerformanceTask, palette: ThemeColors) -> impl IntoElement {
    let diagnostics = task.diagnostics;
    let title = if task.title.trim().is_empty() {
        "Untitled tab".to_owned()
    } else {
        task.title
    };
    let task_type = if task.id == 0 {
        "Photon process"
    } else if task.loading {
        "Loading · WebContent + owned workers"
    } else {
        "WebContent + owned workers"
    };
    v_stack()
        .w_full()
        .gap(px(metrics::PERFORMANCE_TASK_GAP))
        .child(
            v_stack()
                .gap(px(metrics::MENU_ITEM_GAP))
                .child(
                    div()
                        .text_size(px(metrics::INTERNAL_PAGE_BODY_SIZE))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(palette.text_primary))
                        .child(title),
                )
                .child(secondary_text(
                    if task.url.is_empty() {
                        task_type.to_owned()
                    } else {
                        format!("{task_type} · {}", task.url)
                    },
                    palette,
                )),
        )
        .child(
            h_stack()
                .w_full()
                .gap(px(metrics::PERFORMANCE_PAGE_GAP))
                .child(task_metric(
                    "CPU",
                    diagnostics
                        .cpu_percent
                        .map_or_else(|| "—".into(), |cpu| format!("{cpu:.1}%")),
                    palette,
                ))
                .child(task_metric(
                    "Memory",
                    diagnostics
                        .memory_bytes
                        .map_or_else(|| "—".into(), format_bytes),
                    palette,
                ))
                .child(task_metric(
                    "Managed heap",
                    diagnostics
                        .managed_heap_bytes
                        .map_or_else(|| "—".into(), format_bytes),
                    palette,
                ))
                .child(task_metric(
                    "FPS",
                    diagnostics
                        .frames_per_second
                        .map_or_else(|| "—".into(), |fps| format!("{fps:.1} fps")),
                    palette,
                ))
                .child(task_metric(
                    "Network ↓ / ↑",
                    format!(
                        "{} / {}",
                        format_rate(diagnostics.download_bytes_per_second),
                        format_rate(diagnostics.upload_bytes_per_second)
                    ),
                    palette,
                ))
                .child(task_metric(
                    "Input → frame",
                    diagnostics
                        .input_to_frame_latency_ms
                        .map_or_else(|| "—".into(), |value| format!("{value:.1} ms")),
                    palette,
                ))
                .child(task_metric(
                    "Frame gap · 10s",
                    diagnostics
                        .longest_frame_gap_ms
                        .map_or_else(|| "—".into(), |value| format!("{value:.1} ms")),
                    palette,
                )),
        )
}

fn task_metric(label: &'static str, value: String, palette: ThemeColors) -> impl IntoElement {
    v_stack()
        .flex_1()
        .min_w(px(0.0))
        .gap(px(metrics::MENU_ITEM_GAP))
        .child(secondary_text(label, palette))
        .child(
            div()
                .text_size(px(metrics::TAB_FONT_SIZE))
                .text_color(rgb(palette.text_primary))
                .child(value),
        )
}
