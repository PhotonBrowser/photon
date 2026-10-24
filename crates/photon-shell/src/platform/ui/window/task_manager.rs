//! A separate, live view of Photon and every process registered by the Engine.

use gpui::{
    App, Context, FontWeight, Render, Role, Subscription, Task, Window, div, prelude::*, px, rgb,
    rgba, size,
};
use photon_performance::{format_bytes, format_rate};
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    time::{Duration, Instant},
};

use crate::platform::{
    engine::{EngineProcessSnapshot, EngineRuntime, PerformanceDashboardLease},
    ui::{
        layout::{h_stack, secondary_text, v_stack},
        metrics,
        settings::Settings,
        theme::{ThemeColors, palette},
        titlebar::titlebar,
    },
    window_config, window_settings,
};

const SAMPLE_INTERVAL_MS: u64 = 1_000;

struct ProcessIoSample {
    at: Instant,
    read_bytes: u64,
    write_bytes: u64,
}

struct TaskManagerProcess {
    snapshot: EngineProcessSnapshot,
    disk_read_bytes_per_second: Option<u64>,
    disk_write_bytes_per_second: Option<u64>,
}

pub(super) struct TaskManagerWindow {
    runtime: Rc<EngineRuntime>,
    _dashboard: PerformanceDashboardLease,
    processes: Vec<TaskManagerProcess>,
    previous_disk_samples: HashMap<u64, ProcessIoSample>,
    _sample_task: Task<()>,
    _settings_subscription: Subscription,
    _appearance_subscription: Subscription,
}

impl TaskManagerWindow {
    fn new(runtime: Rc<EngineRuntime>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings_subscription = cx.observe_global_in::<Settings>(window, |_, window, cx| {
            window_settings::configure_native_controls(window, &Settings::get(cx).animations);
            cx.notify();
        });
        let appearance_subscription = cx.observe_window_appearance(window, |_, _, cx| cx.notify());
        let dashboard = runtime.acquire_performance_dashboard();
        let mut task_manager = Self {
            runtime,
            _dashboard: dashboard,
            processes: Vec::new(),
            previous_disk_samples: HashMap::new(),
            _sample_task: cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(SAMPLE_INTERVAL_MS))
                        .await;
                    if this
                        .update(cx, |task_manager, cx| {
                            task_manager.sample();
                            cx.notify();
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            }),
            _settings_subscription: settings_subscription,
            _appearance_subscription: appearance_subscription,
        };
        task_manager.sample();
        task_manager
    }

    fn sample(&mut self) {
        let now = Instant::now();
        let snapshots = self.runtime.process_snapshots();
        let mut live_processes = HashSet::with_capacity(snapshots.len());
        self.processes = snapshots
            .into_iter()
            .map(|snapshot| {
                let (disk_read_bytes_per_second, disk_write_bytes_per_second) =
                    match (snapshot.disk_read_bytes, snapshot.disk_write_bytes) {
                        (Some(read_bytes), Some(write_bytes)) => {
                            let rates = self
                                .previous_disk_samples
                                .get(&snapshot.child_id)
                                .and_then(|previous| {
                                    let elapsed = now.duration_since(previous.at).as_secs_f64();
                                    if elapsed > 0.0 {
                                        Some((
                                            rate(previous.read_bytes, read_bytes, elapsed),
                                            rate(previous.write_bytes, write_bytes, elapsed),
                                        ))
                                    } else {
                                        None
                                    }
                                });
                            self.previous_disk_samples.insert(
                                snapshot.child_id,
                                ProcessIoSample {
                                    at: now,
                                    read_bytes,
                                    write_bytes,
                                },
                            );
                            rates.unwrap_or((None, None))
                        }
                        _ => {
                            self.previous_disk_samples.remove(&snapshot.child_id);
                            (None, None)
                        }
                    };
                live_processes.insert(snapshot.child_id);
                TaskManagerProcess {
                    snapshot,
                    disk_read_bytes_per_second,
                    disk_write_bytes_per_second,
                }
            })
            .collect();
        self.previous_disk_samples
            .retain(|child_id, _| live_processes.contains(child_id));
        self.processes.sort_by(|left, right| {
            right
                .snapshot
                .cpu_percent
                .unwrap_or_default()
                .total_cmp(&left.snapshot.cpu_percent.unwrap_or_default())
                .then_with(|| {
                    right
                        .snapshot
                        .memory_bytes
                        .unwrap_or_default()
                        .cmp(&left.snapshot.memory_bytes.unwrap_or_default())
                })
                .then_with(|| left.snapshot.child_id.cmp(&right.snapshot.child_id))
        });
    }

    fn total_cpu_percent(&self) -> Option<f64> {
        let mut total = 0.0;
        for process in &self.processes {
            total += process.snapshot.cpu_percent?;
        }
        (!self.processes.is_empty()).then_some(total)
    }

    fn total_memory_bytes(&self) -> Option<u64> {
        let mut total = 0_u64;
        for process in &self.processes {
            total = total.checked_add(process.snapshot.memory_bytes?)?;
        }
        (!self.processes.is_empty()).then_some(total)
    }

    fn total_disk_rates(&self) -> Option<(u64, u64)> {
        let mut read = 0_u64;
        let mut write = 0_u64;
        for process in &self.processes {
            read = read.checked_add(process.disk_read_bytes_per_second?)?;
            write = write.checked_add(process.disk_write_bytes_per_second?)?;
        }
        (!self.processes.is_empty()).then_some((read, write))
    }

    fn total_network_rates(&self) -> Option<(u64, u64)> {
        let mut download = 0_u64;
        let mut upload = 0_u64;
        for process in &self.processes {
            download = download.checked_add(process.snapshot.download_bytes_per_second?)?;
            upload = upload.checked_add(process.snapshot.upload_bytes_per_second?)?;
        }
        (!self.processes.is_empty()).then_some((download, upload))
    }

    fn summary(&self, palette: ThemeColors) -> impl IntoElement {
        h_stack()
            .w_full()
            .gap(px(metrics::TASK_MANAGER_CARD_GAP))
            .child(summary_card(
                "CPU",
                self.total_cpu_percent()
                    .map_or_else(|| "—".into(), |value| format!("{value:.1}%")),
                "All Engine processes",
                palette,
            ))
            .child(summary_card(
                "Memory",
                self.total_memory_bytes()
                    .map_or_else(|| "—".into(), format_bytes),
                "Combined process memory",
                palette,
            ))
            .child(summary_card(
                "Disk read / write",
                self.total_disk_rates().map_or_else(
                    || "—".into(),
                    |(read, write)| format!("{} / {}", format_rate(read), format_rate(write)),
                ),
                "Per-process I/O rates",
                palette,
            ))
            .child(summary_card(
                "Network down / up",
                self.total_network_rates().map_or_else(
                    || "—".into(),
                    |(download, upload)| {
                        format!("{} / {}", format_rate(download), format_rate(upload))
                    },
                ),
                "Engine request traffic",
                palette,
            ))
    }

    fn table(&self, palette: ThemeColors) -> impl IntoElement {
        let rows = self
            .processes
            .iter()
            .map(|process| process_row(process, palette));
        v_stack()
            .w_full()
            .flex_1()
            .min_h_0()
            .gap(px(metrics::MENU_ITEM_GAP))
            .child(
                h_stack()
                    .id("task-manager-table-header")
                    .w_full()
                    .h(px(metrics::TASK_MANAGER_TABLE_HEADER_HEIGHT))
                    .items_center()
                    .gap(px(metrics::TASK_MANAGER_COLUMN_GAP))
                    .px(px(metrics::TASK_MANAGER_CARD_PADDING))
                    .rounded(px(metrics::surface_radius()))
                    .bg(rgba(palette.menu_surface))
                    .child(column_heading("Task", None, palette))
                    .child(column_heading(
                        "PID",
                        Some(metrics::TASK_MANAGER_PID_WIDTH),
                        palette,
                    ))
                    .child(column_heading(
                        "CPU",
                        Some(metrics::TASK_MANAGER_CPU_WIDTH),
                        palette,
                    ))
                    .child(column_heading(
                        "Memory",
                        Some(metrics::TASK_MANAGER_MEMORY_WIDTH),
                        palette,
                    ))
                    .child(column_heading(
                        "Disk read / write",
                        Some(metrics::TASK_MANAGER_DISK_WIDTH),
                        palette,
                    ))
                    .child(column_heading(
                        "Network down / up",
                        Some(metrics::TASK_MANAGER_NETWORK_WIDTH),
                        palette,
                    )),
            )
            .child(
                div()
                    .id("task-manager-processes")
                    .aria_label("Engine processes")
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_y_scroll()
                    .child(
                        v_stack()
                            .w_full()
                            .gap(px(metrics::MENU_ITEM_GAP))
                            .children(rows),
                    ),
            )
    }
}

impl Render for TaskManagerWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = palette(window, cx);
        v_stack()
            .id("task-manager-window")
            .size_full()
            .bg(palette.window_background())
            .text_color(rgb(palette.text_primary))
            .child(titlebar(
                div()
                    .w_full()
                    .text_size(px(metrics::INTERNAL_PAGE_BODY_SIZE))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(palette.text_primary))
                    .child("Task Manager"),
                window,
                palette,
            ))
            .child(
                v_stack()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .p(px(metrics::TASK_MANAGER_INSET))
                    .gap(px(metrics::TASK_MANAGER_SECTION_GAP))
                    .child(
                        v_stack()
                            .gap(px(metrics::MENU_ITEM_GAP))
                            .child(
                                div()
                                    .id("task-manager-total-usage-heading")
                                    .role(Role::Heading)
                                    .aria_level(1)
                                    .text_size(px(metrics::INTERNAL_PAGE_SECTION_SIZE))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(palette.text_primary))
                                    .child("Total app usage"),
                            )
                            .child(secondary_text(
                                "Live usage across Photon and every process registered by the Engine.",
                                palette,
                            )),
                    )
                    .child(self.summary(palette))
                    .child(
                        h_stack()
                            .w_full()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .id("task-manager-processes-heading")
                                    .role(Role::Heading)
                                    .aria_level(2)
                                    .text_size(px(metrics::INTERNAL_PAGE_BODY_SIZE))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(palette.text_primary))
                                    .child(format!("Processes · {}", self.processes.len())),
                            )
                            .child(secondary_text(
                                "Refreshes every second · CPU can exceed 100% across cores",
                                palette,
                            )),
                    )
                    .child(self.table(palette))
                    .when(self.processes.is_empty(), |content| {
                        content.child(secondary_text(
                            "No Engine process data is available in this build.",
                            palette,
                        ))
                    }),
            )
    }
}

pub(super) fn open_task_manager(runtime: Rc<EngineRuntime>, cx: &mut App) {
    if let Some(existing) = cx
        .windows()
        .into_iter()
        .find_map(|handle| handle.downcast::<TaskManagerWindow>())
    {
        let _ = existing.update(cx, |_, window, _| window.activate());
        return;
    }

    let mut options = window_settings::popup_options(
        cx,
        Some(window_config::TASK_MANAGER_WIDTH as i32),
        Some(window_config::TASK_MANAGER_HEIGHT as i32),
    );
    options.window_min_size = Some(size(
        px(window_config::TASK_MANAGER_MIN_WIDTH),
        px(window_config::TASK_MANAGER_MIN_HEIGHT),
    ));
    let animations = Settings::get(cx).animations;
    if let Err(error) = cx.open_window(options, move |window, cx| {
        window_settings::configure_native_controls(window, &animations);
        window.set_window_title("Task Manager");
        cx.new(|cx| TaskManagerWindow::new(runtime, window, cx))
    }) {
        crate::platform::trace(format_args!("task manager window: {error:#}"));
    }
}

fn summary_card(
    label: &'static str,
    value: String,
    detail: &'static str,
    palette: ThemeColors,
) -> impl IntoElement {
    v_stack()
        .flex_1()
        .min_w(px(0.0))
        .p(px(metrics::TASK_MANAGER_CARD_PADDING))
        .gap(px(metrics::MENU_ITEM_GAP))
        .rounded(px(metrics::surface_radius()))
        .border_1()
        .border_color(rgba(palette.menu_border))
        .bg(rgba(palette.menu_surface))
        .child(secondary_text(label, palette))
        .child(
            div()
                .text_size(px(metrics::INTERNAL_PAGE_BODY_SIZE))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(palette.text_primary))
                .child(value),
        )
        .child(secondary_text(detail, palette))
}

fn column_heading(
    label: &'static str,
    width: Option<f32>,
    palette: ThemeColors,
) -> impl IntoElement {
    div()
        .when_some(width, |heading, width| heading.w(px(width)))
        .when(width.is_none(), |heading| {
            heading
                .flex_1()
                .min_w(px(metrics::TASK_MANAGER_PROCESS_MIN_WIDTH))
        })
        .text_size(px(metrics::TAB_FONT_SIZE))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(palette.text_secondary))
        .child(label)
}

fn process_row(process: &TaskManagerProcess, palette: ThemeColors) -> impl IntoElement {
    let snapshot = &process.snapshot;
    let title = snapshot
        .title
        .as_deref()
        .filter(|title| !title.trim().is_empty());
    let page_url = snapshot
        .page_url
        .as_deref()
        .filter(|url| !url.trim().is_empty());
    let subtitle = match (title, page_url) {
        (Some(title), Some(url)) if title != url => format!("{title} · {url}"),
        (Some(title), _) => title.to_owned(),
        (None, Some(url)) => url.to_owned(),
        (None, None) => format!("Process {}", snapshot.pid),
    };
    let role = process_role(&snapshot.role, snapshot.page_is_frame);
    h_stack()
        .id(format!("task-manager-process-{}", snapshot.child_id))
        .w_full()
        .h(px(metrics::TASK_MANAGER_TABLE_ROW_HEIGHT))
        .items_center()
        .gap(px(metrics::TASK_MANAGER_COLUMN_GAP))
        .px(px(metrics::TASK_MANAGER_CARD_PADDING))
        .rounded(px(metrics::surface_radius()))
        .border_1()
        .border_color(rgba(palette.menu_border))
        .bg(rgba(palette.surface))
        .hover_style(|style| style.bg(rgba(palette.hover_surface)))
        .child(
            v_stack()
                .flex_1()
                .min_w(px(metrics::TASK_MANAGER_PROCESS_MIN_WIDTH))
                .gap(px(metrics::MENU_ITEM_GAP))
                .child(
                    div()
                        .text_size(px(metrics::TAB_FONT_SIZE))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(palette.text_primary))
                        .child(role),
                )
                .child(
                    div()
                        .w_full()
                        .text_size(px(metrics::TAB_FONT_SIZE))
                        .text_color(rgb(palette.text_secondary))
                        .truncate()
                        .child(subtitle),
                ),
        )
        .child(metric_cell(
            snapshot.pid.to_string(),
            metrics::TASK_MANAGER_PID_WIDTH,
            palette,
        ))
        .child(metric_cell(
            snapshot
                .cpu_percent
                .map_or_else(|| "—".into(), |value| format!("{value:.1}%")),
            metrics::TASK_MANAGER_CPU_WIDTH,
            palette,
        ))
        .child(metric_cell(
            snapshot
                .memory_bytes
                .map_or_else(|| "—".into(), format_bytes),
            metrics::TASK_MANAGER_MEMORY_WIDTH,
            palette,
        ))
        .child(metric_cell(
            format_pair(
                process.disk_read_bytes_per_second,
                process.disk_write_bytes_per_second,
            ),
            metrics::TASK_MANAGER_DISK_WIDTH,
            palette,
        ))
        .child(metric_cell(
            format_pair(
                snapshot.download_bytes_per_second,
                snapshot.upload_bytes_per_second,
            ),
            metrics::TASK_MANAGER_NETWORK_WIDTH,
            palette,
        ))
}

fn process_role(role: &str, is_frame: bool) -> String {
    let name = match role {
        "Browser" => photon_brand::NAME,
        "Compositor" => "Compositor",
        "WebContent" => "Web Content",
        "WebWorker" => "Web Worker",
        "RequestServer" => "Network Service",
        "ImageDecoder" => "Image Decoder",
        "MediaServer" => "Media Service",
        "AudioServer" => "Audio Service",
        "VideoDecoderServer" => "Video Decoder",
        other => other,
    };
    if is_frame {
        format!("{name} · Subframe")
    } else {
        name.to_owned()
    }
}

fn metric_cell(value: String, width: f32, palette: ThemeColors) -> impl IntoElement {
    div()
        .w(px(width))
        .text_size(px(metrics::TAB_FONT_SIZE))
        .text_color(rgb(palette.text_primary))
        .text_right()
        .child(value)
}

fn format_pair(first: Option<u64>, second: Option<u64>) -> String {
    match (first, second) {
        (Some(first), Some(second)) => format!("{} / {}", format_rate(first), format_rate(second)),
        _ => "—".into(),
    }
}

fn rate(previous: u64, current: u64, elapsed_seconds: f64) -> Option<u64> {
    let delta = current.checked_sub(previous)?;
    Some((delta as f64 / elapsed_seconds).round() as u64)
}
