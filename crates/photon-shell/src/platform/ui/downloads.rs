//! The profile's downloads, shared by every window. Engine reports them; the
//! downloads page lists them and observes this global to follow changes.

use gpui::{App, BorrowAppContext, Global};

use super::super::engine::{Download, DownloadChange, EngineRuntime};
use crate::platform::app_update::update_app;

/// Downloads, newest first.
#[derive(Default)]
pub(super) struct Downloads {
    downloads: Vec<Download>,
}

impl Global for Downloads {}

impl Downloads {
    pub(super) fn list(cx: &App) -> &[Download] {
        cx.try_global::<Self>()
            .map_or(&[], |downloads| downloads.downloads.as_slice())
    }

    fn apply(&mut self, change: DownloadChange) {
        match change {
            DownloadChange::Changed(download) => {
                if let Some(existing) = self.downloads.iter_mut().find(|d| d.id == download.id) {
                    *existing = download;
                } else {
                    self.downloads.push(download);
                    self.downloads
                        .sort_by_key(|download| std::cmp::Reverse(download.created_time_ms));
                }
            }
            DownloadChange::Removed(id) => self.downloads.retain(|download| download.id != id),
        }
    }
}

/// Follows Engine's download list for the life of the app.
pub(in crate::platform) fn follow_downloads(runtime: &EngineRuntime, cx: &mut App) {
    cx.set_global(Downloads::default());
    let app = cx.to_async();
    runtime.on_download_change(move |change| {
        app.spawn(async move |cx| {
            update_app(cx, |cx| {
                cx.update_global::<Downloads, _>(|downloads, _| downloads.apply(change))
            })
            .await;
        })
        .detach();
    });
}
