//! The profile's downloads, shared by every window. Engine reports them; the
//! sidebar's downloads button and popover, the window's notices and the
//! downloads page observe this global to follow changes.

use gpui::{App, BorrowAppContext, Global};
use photon_performance::{format_bytes, format_rate};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use super::super::engine::{Download, DownloadChange, DownloadStatus, EngineRuntime};
use crate::platform::app_update::update_app;

/// Something that just happened to a download, which the window announces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DownloadEvent {
    Started(u64),
    Finished(u64),
    Failed(u64),
}

/// Downloads, newest first. Private windows list only private downloads and
/// other windows only the rest, so each kind keeps its own news.
#[derive(Default)]
pub(super) struct Downloads {
    downloads: Vec<Download>,
    /// For other windows, then private ones: the latest start, finish or
    /// failure, and when it happened.
    latest_event: [Option<(DownloadEvent, Instant)>; 2],
    /// For other windows, then private ones: whether a download arrived since
    /// the downloads popover was last open.
    unseen: [bool; 2],
}

/// How far along the downloads in progress are, together.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct DownloadProgress {
    /// The share received, or `None` while a size is unknown.
    pub(super) fraction: Option<f32>,
}

impl Global for Downloads {}

impl Downloads {
    /// The downloads a private window, or another window, lists.
    pub(super) fn list(private: bool, cx: &App) -> impl Iterator<Item = &Download> {
        cx.try_global::<Self>()
            .into_iter()
            .flat_map(|downloads| downloads.downloads.iter())
            .filter(move |download| download.private == private)
    }

    pub(super) fn get(id: u64, cx: &App) -> Option<&Download> {
        cx.try_global::<Self>()?
            .downloads
            .iter()
            .find(|download| download.id == id)
    }

    pub(super) fn latest_event(private: bool, cx: &App) -> Option<(DownloadEvent, Instant)> {
        cx.try_global::<Self>()?.latest_event[usize::from(private)]
    }

    pub(super) fn has_unseen(private: bool, cx: &App) -> bool {
        cx.try_global::<Self>()
            .is_some_and(|downloads| downloads.unseen[usize::from(private)])
    }

    /// The downloads popover was opened, so nothing in it is new any more.
    pub(super) fn mark_seen(private: bool, cx: &mut App) {
        if Self::has_unseen(private, cx) {
            cx.update_global::<Self, _>(|downloads, _| {
                downloads.unseen[usize::from(private)] = false;
            });
        }
    }

    /// The progress of the downloads in progress, or `None` when there are none.
    pub(super) fn progress(private: bool, cx: &App) -> Option<DownloadProgress> {
        let active: Vec<_> = Self::list(private, cx)
            .filter(|download| download.status.is_active())
            .collect();
        if active.is_empty() {
            return None;
        }
        let total = active
            .iter()
            .map(|download| download.total_bytes.filter(|total| *total > 0))
            .sum::<Option<u64>>();
        let received: u64 = active.iter().map(|download| download.received_bytes).sum();
        Some(DownloadProgress {
            fraction: total.map(|total| (received as f32 / total as f32).clamp(0.0, 1.0)),
        })
    }

    fn would_change(&self, change: &DownloadChange) -> bool {
        match change {
            DownloadChange::Changed(download) => self
                .downloads
                .iter()
                .find(|existing| existing.id == download.id)
                .is_none_or(|existing| existing != download),
            DownloadChange::Removed(id) => self.downloads.iter().any(|download| download.id == *id),
        }
    }

    fn apply(&mut self, changes: impl IntoIterator<Item = DownloadChange>) {
        let mut added = false;
        for change in changes {
            match change {
                DownloadChange::Changed(download) => {
                    let previous = self
                        .downloads
                        .iter()
                        .find(|d| d.id == download.id)
                        .map(|existing| existing.status);
                    let kind = usize::from(download.private);
                    if let Some(event) = event_for(previous, &download) {
                        self.latest_event[kind] = Some((event, Instant::now()));
                    }
                    if let Some(existing) = self.downloads.iter_mut().find(|d| d.id == download.id)
                    {
                        *existing = download;
                    } else {
                        self.downloads.push(download);
                        self.unseen[kind] = true;
                        added = true;
                    }
                }
                DownloadChange::Removed(id) => self.downloads.retain(|download| download.id != id),
            }
        }
        if added {
            self.downloads
                .sort_by_key(|download| std::cmp::Reverse(download.created_time_ms));
        }
    }
}

/// Follows Engine's download list for the life of the app.
pub(in crate::platform) fn follow_downloads(runtime: &EngineRuntime, cx: &mut App) {
    cx.set_global(Downloads::default());
    let app = cx.to_async();
    let pending = Rc::new(RefCell::new(HashMap::<u64, DownloadChange>::new()));
    let scheduled = Rc::new(Cell::new(false));
    runtime.on_download_change(move |change| {
        let id = match &change {
            DownloadChange::Changed(download) => download.id,
            DownloadChange::Removed(id) => *id,
        };
        pending.borrow_mut().insert(id, change);
        if scheduled.replace(true) {
            return;
        }

        let pending = pending.clone();
        let scheduled = scheduled.clone();
        app.spawn(async move |cx| {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let changes = std::mem::take(&mut *pending.borrow_mut())
                .into_values()
                .collect::<Vec<_>>();
            scheduled.set(false);
            update_app(cx, move |cx| {
                let changed = cx.try_global::<Downloads>().is_some_and(|downloads| {
                    changes.iter().any(|change| downloads.would_change(change))
                });
                if changed {
                    cx.update_global::<Downloads, _>(|downloads, _| downloads.apply(changes));
                }
            })
            .await;
        })
        .detach();
    });
}

/// What a download's change from `previous` (`None` when it is new) means to
/// announce. Downloads Engine restores from an earlier session are not.
fn event_for(previous: Option<DownloadStatus>, download: &Download) -> Option<DownloadEvent> {
    let id = download.id;
    match (previous, download.status) {
        (None, DownloadStatus::InProgress) => Some(DownloadEvent::Started(id)),
        (Some(previous), DownloadStatus::Completed) if previous.is_active() => {
            Some(DownloadEvent::Finished(id))
        }
        (Some(previous), DownloadStatus::Failed) if previous.is_active() => {
            Some(DownloadEvent::Failed(id))
        }
        // A small file can finish before Engine first reports it.
        (None, DownloadStatus::Completed) if is_recent(download) => {
            Some(DownloadEvent::Finished(id))
        }
        _ => None,
    }
}

/// Whether the download began moments ago, rather than in an earlier session.
fn is_recent(download: &Download) -> bool {
    const RECENT: Duration = Duration::from_secs(30);
    now_ms().saturating_sub(download.created_time_ms) < RECENT.as_millis() as i64
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as i64)
}

/// The downloaded file's name, or its address before it has one.
pub(super) fn file_name(download: &Download) -> String {
    download
        .path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| download.url.clone())
}

/// How much of the file has arrived, while its size is known.
pub(super) fn progress_fraction(download: &Download) -> Option<f32> {
    download
        .total_bytes
        .filter(|total| *total > 0)
        .map(|total| (download.received_bytes as f32 / total as f32).clamp(0.0, 1.0))
}

/// What a download's row says under its name.
pub(super) fn status_text(download: &Download) -> String {
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
        DownloadStatus::Completed => {
            let size = size.unwrap_or(received);
            format!("{size} · {}", time_ago(download.created_time_ms, now_ms()))
        }
        DownloadStatus::Canceled => "Cancelled".into(),
        DownloadStatus::Failed => match &download.error {
            Some(error) => format!("Failed · {error}"),
            None => "Failed".into(),
        },
    }
}

/// How long ago `then_ms` was, as "17 minutes ago".
fn time_ago(then_ms: i64, now_ms: i64) -> String {
    let seconds = (now_ms - then_ms).max(0) / 1000;
    let (count, unit) = match seconds {
        0..60 => return "Just now".into(),
        60..3_600 => (seconds / 60, "minute"),
        3_600..86_400 => (seconds / 3_600, "hour"),
        _ => (seconds / 86_400, "day"),
    };
    let plural = if count == 1 { "" } else { "s" };
    format!("{count} {unit}{plural} ago")
}

/// The kind of file a download is, for its icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FileKind {
    Document,
    Image,
    Video,
    Audio,
    Archive,
    Code,
    Other,
}

impl FileKind {
    pub(super) fn of(download: &Download) -> Self {
        let extension = download
            .path
            .extension()
            .map(|extension| extension.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        match extension.as_str() {
            "pdf" | "txt" | "md" | "rtf" | "doc" | "docx" | "csv" | "xls" | "xlsx" | "ppt"
            | "pptx" | "pages" | "numbers" | "key" | "odt" | "ods" => Self::Document,
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "heic" | "avif" | "bmp" | "tiff" => {
                Self::Image
            }
            "mp4" | "mov" | "mkv" | "webm" | "avi" | "m4v" => Self::Video,
            "mp3" | "wav" | "m4a" | "flac" | "aac" | "ogg" | "opus" => Self::Audio,
            "zip" | "dmg" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "7z" | "rar" | "pkg" | "iso" => {
                Self::Archive
            }
            "html" | "htm" | "json" | "js" | "ts" | "css" | "xml" | "rs" | "py" | "sh" => {
                Self::Code
            }
            _ => Self::Other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn download(status: DownloadStatus, created_time_ms: i64) -> Download {
        Download {
            id: 1,
            url: "https://example.com/report.csv".into(),
            path: PathBuf::from("/tmp/report.csv"),
            status,
            received_bytes: 0,
            total_bytes: None,
            bytes_per_second: None,
            error: None,
            created_time_ms,
            private: false,
        }
    }

    #[test]
    fn private_downloads_keep_their_own_news() {
        let mut downloads = Downloads::default();
        let private = Download {
            id: 2,
            private: true,
            ..download(DownloadStatus::InProgress, now_ms())
        };
        downloads.apply([DownloadChange::Changed(private)]);
        assert!(downloads.unseen[1]);
        assert!(!downloads.unseen[0]);
        assert!(downloads.latest_event[1].is_some());
        assert!(downloads.latest_event[0].is_none());
    }

    #[test]
    fn announces_starts_finishes_and_failures() {
        let now = now_ms();
        assert_eq!(
            event_for(None, &download(DownloadStatus::InProgress, now)),
            Some(DownloadEvent::Started(1))
        );
        assert_eq!(
            event_for(
                Some(DownloadStatus::InProgress),
                &download(DownloadStatus::Completed, now)
            ),
            Some(DownloadEvent::Finished(1))
        );
        assert_eq!(
            event_for(
                Some(DownloadStatus::Paused),
                &download(DownloadStatus::Failed, now)
            ),
            Some(DownloadEvent::Failed(1))
        );
        assert_eq!(
            event_for(
                Some(DownloadStatus::InProgress),
                &download(DownloadStatus::InProgress, now)
            ),
            None
        );
    }

    #[test]
    fn restored_downloads_are_not_announced() {
        let earlier = now_ms() - 60 * 60 * 1000;
        assert_eq!(
            event_for(None, &download(DownloadStatus::Completed, earlier)),
            None
        );
        assert_eq!(
            event_for(None, &download(DownloadStatus::Completed, now_ms())),
            Some(DownloadEvent::Finished(1))
        );
    }

    #[test]
    fn time_ago_rounds_down_to_the_largest_unit() {
        let minute = 60_000;
        assert_eq!(time_ago(0, 30_000), "Just now");
        assert_eq!(time_ago(0, minute), "1 minute ago");
        assert_eq!(time_ago(0, 17 * minute), "17 minutes ago");
        assert_eq!(time_ago(0, 6 * 60 * minute), "6 hours ago");
        assert_eq!(time_ago(0, 3 * 24 * 60 * minute), "3 days ago");
    }

    #[test]
    fn file_kinds_follow_the_extension() {
        let mut item = download(DownloadStatus::Completed, 0);
        assert_eq!(FileKind::of(&item), FileKind::Document);
        item.path = PathBuf::from("/tmp/Rex.DMG");
        assert_eq!(FileKind::of(&item), FileKind::Archive);
        item.path = PathBuf::from("/tmp/notes");
        assert_eq!(FileKind::of(&item), FileKind::Other);
    }
}
