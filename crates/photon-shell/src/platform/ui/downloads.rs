//! The profile's downloads, shared by every window. Engine reports them; the
//! downloads page lists them and observes this global to follow changes.

use gpui::{App, BorrowAppContext, Global};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    time::Duration,
};

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
                    if let Some(existing) = self.downloads.iter_mut().find(|d| d.id == download.id)
                    {
                        *existing = download;
                    } else {
                        self.downloads.push(download);
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
