//! `photon://downloads`: the files Engine saved for pages, with their progress,
//! and ways to pause, cancel, open or find them.

use gpui::{
    Context, Render, Subscription, Window, div, prelude::*, px, relative, rgb, rgba, uniform_list,
};
use photon_performance::{format_bytes, format_rate};
use std::rc::Rc;

use super::super::super::engine::{Download, DownloadStatus, EngineRuntime};
use super::super::button::{ButtonKind, ButtonSize, button};
use super::super::downloads::Downloads;
use super::super::icons::download_icon;
use super::super::layout::{h_stack, v_stack};
use super::super::{metrics, theme::ThemeColors, theme::page_palette as palette};
use super::layout::{group, heading, secondary_text};
use super::{PageContext, PageDefinition, PageIcon};

pub(super) const PAGE: PageDefinition = PageDefinition {
    name: "downloads",
    title: "Downloads",
    icon: PageIcon::Symbol(|color, size| download_icon(color, size).into_any_element()),
    shows_address: true,
    single_tab: true,
    build: |context, cx| cx.new(|cx| DownloadsPage::new(context, cx)).into(),
};

pub(in super::super) struct DownloadsPage {
    runtime: Rc<EngineRuntime>,
    _downloads: Subscription,
}

impl DownloadsPage {
    fn new(context: PageContext, cx: &mut Context<Self>) -> Self {
        Self {
            runtime: context.runtime,
            _downloads: cx.observe_global::<Downloads>(|_, cx| cx.notify()),
        }
    }
}

/// What the row says under the file's name.
fn status_text(download: &Download) -> String {
    let received = format_bytes(download.received_bytes);
    let size = download.total_bytes.map(format_bytes);
    match download.status {
        DownloadStatus::InProgress => {
            let progress = size.map_or(received.clone(), |size| format!("{received} of {size}"));
            match download.bytes_per_second {
                Some(rate) => format!("{progress} · {}", format_rate(rate)),
                None => progress,
            }
        }
        DownloadStatus::Paused => size.map_or_else(
            || format!("Paused · {received}"),
            |size| format!("Paused · {received} of {size}"),
        ),
        DownloadStatus::Completed => size.unwrap_or(received),
        DownloadStatus::Canceled => "Cancelled".into(),
        DownloadStatus::Failed => match &download.error {
            Some(error) => format!("Failed · {error}"),
            None => "Failed".into(),
        },
    }
}

fn file_name(download: &Download) -> String {
    download
        .path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| download.url.clone())
}

fn progress_bar(download: &Download, palette: ThemeColors) -> impl IntoElement {
    if !download.status.is_active() {
        return div().h(px(metrics::DOWNLOAD_PROGRESS_HEIGHT));
    }
    let fraction = download
        .total_bytes
        .filter(|total| *total > 0)
        .map_or(0.0, |total| download.received_bytes as f32 / total as f32)
        .clamp(0.0, 1.0);
    div()
        .w_full()
        .h(px(metrics::DOWNLOAD_PROGRESS_HEIGHT))
        .rounded_full()
        .bg(rgba(palette.menu_border))
        .child(
            div()
                .h_full()
                .w(relative(fraction))
                .rounded_full()
                .bg(rgb(palette.chosen)),
        )
}

impl DownloadsPage {
    fn row(
        runtime: &Rc<EngineRuntime>,
        download: &Download,
        palette: ThemeColors,
    ) -> impl IntoElement {
        let id = download.id;
        let mut actions = h_stack().gap(px(metrics::MENU_ITEM_GAP)).flex_shrink_0();
        let action = |name: &'static str, kind: ButtonKind, handler: super::super::ClickHandler| {
            button(
                gpui::ElementId::Name(format!("download-{name}-{id}").into()),
                name,
                kind,
                true,
                ButtonSize::Small,
                palette,
                handler,
            )
        };
        match download.status {
            DownloadStatus::InProgress | DownloadStatus::Paused => {
                let resume_runtime = runtime.clone();
                let paused = download.status == DownloadStatus::Paused;
                actions = actions.child(action(
                    if paused { "Resume" } else { "Pause" },
                    ButtonKind::Secondary,
                    Box::new(move |_, _, _| {
                        if paused {
                            resume_runtime.resume_download(id);
                        } else {
                            resume_runtime.pause_download(id);
                        }
                    }),
                ));
                let cancel_runtime = runtime.clone();
                actions = actions.child(action(
                    "Cancel",
                    ButtonKind::Destructive,
                    Box::new(move |_, _, _| cancel_runtime.cancel_download(id)),
                ));
            }
            DownloadStatus::Completed => {
                let path = download.path.clone();
                actions = actions.child(action(
                    "Open",
                    ButtonKind::Secondary,
                    Box::new(move |_, _, cx| cx.open_with_system(&path)),
                ));
                let path = download.path.clone();
                actions = actions.child(action(
                    "Show in Folder",
                    ButtonKind::Secondary,
                    Box::new(move |_, _, cx| cx.reveal_path(&path)),
                ));
            }
            DownloadStatus::Canceled | DownloadStatus::Failed => {}
        }
        h_stack()
            .h(px(metrics::DOWNLOAD_ROW_HEIGHT))
            .w_full()
            .items_center()
            .gap(px(metrics::LIST_ROW_GAP))
            .p(px(metrics::LIST_ROW_PADDING))
            .child(download_icon(
                palette.text_secondary,
                metrics::LIST_ROW_ICON_SIZE,
            ))
            .child(
                v_stack()
                    .flex_1()
                    .min_w_0()
                    .gap(px(metrics::MENU_ITEM_GAP * 2.0))
                    .child(
                        div()
                            .truncate()
                            .text_size(px(metrics::MENU_FONT_SIZE))
                            .text_color(rgb(palette.text_primary))
                            .child(file_name(download)),
                    )
                    .child(progress_bar(download, palette))
                    .child(secondary_text(status_text(download), palette).truncate()),
            )
            .child(actions)
    }
}

impl Render for DownloadsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = palette(window, cx);
        let downloads = Downloads::list(cx);
        let has_inactive = downloads
            .iter()
            .any(|download| !download.status.is_active());
        let runtime = self.runtime.clone();
        let header = h_stack()
            .w_full()
            .items_end()
            .justify_between()
            .gap(px(metrics::LIST_ROW_GAP))
            .child(heading(
                "downloads-heading",
                "Downloads",
                "Files you download are saved to your Downloads folder.",
                palette,
            ))
            .child(button(
                "downloads-clear",
                "Clear List",
                ButtonKind::Secondary,
                has_inactive,
                ButtonSize::Small,
                palette,
                Box::new(move |_, _, _| runtime.clear_inactive_downloads()),
            ));
        let list: gpui::AnyElement = if downloads.is_empty() {
            group(palette)
                .p(px(metrics::LIST_ROW_PADDING))
                .child(secondary_text("Nothing downloaded yet.", palette))
                .into_any_element()
        } else {
            let runtime = self.runtime.clone();
            group(palette)
                .p(px(0.))
                .h_full()
                .child(
                    uniform_list(
                        "downloads-list",
                        downloads.len(),
                        move |range, _window, app| {
                            range
                                .map(|index| {
                                    let Some(download) = Downloads::list(app).get(index) else {
                                        return div().into_any_element();
                                    };
                                    DownloadsPage::row(&runtime, download, palette)
                                        .into_any_element()
                                })
                                .collect()
                        },
                    )
                    .w_full()
                    .h_full(),
                )
                .into_any_element()
        };
        v_stack()
            .id("downloads-page")
            .size_full()
            .items_center()
            .overflow_hidden()
            .p(px(metrics::INTERNAL_PAGE_INSET))
            .child(
                v_stack()
                    .size_full()
                    .max_w(px(metrics::INTERNAL_PAGE_MAX_WIDTH))
                    .gap(px(metrics::INTERNAL_PAGE_SECTION_GAP))
                    .child(header)
                    .child(div().flex_1().min_h(px(0.)).w_full().child(list)),
            )
    }
}
