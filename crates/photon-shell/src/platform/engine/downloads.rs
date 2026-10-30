//! The profile's downloads, reported by Engine and controlled from the shell.

use std::ffi::{CStr, c_char, c_void};
use std::path::PathBuf;

use super::EngineRuntime;
use crate::platform::engine_ffi::embedder;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::platform) enum DownloadStatus {
    InProgress,
    Paused,
    Completed,
    Canceled,
    Failed,
}

impl DownloadStatus {
    pub(in crate::platform) fn is_active(self) -> bool {
        matches!(self, Self::InProgress | Self::Paused)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::platform) struct Download {
    pub(in crate::platform) id: u64,
    pub(in crate::platform) url: String,
    pub(in crate::platform) path: PathBuf,
    pub(in crate::platform) status: DownloadStatus,
    pub(in crate::platform) received_bytes: u64,
    pub(in crate::platform) total_bytes: Option<u64>,
    pub(in crate::platform) bytes_per_second: Option<u64>,
    pub(in crate::platform) error: Option<String>,
    pub(in crate::platform) created_time_ms: i64,
}

/// What Engine reports about the download list.
pub(in crate::platform) enum DownloadChange {
    Changed(Download),
    Removed(u64),
}

pub(super) type DownloadCallback = Box<dyn Fn(DownloadChange)>;

fn string(pointer: *const c_char) -> String {
    if pointer.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(pointer) }
        .to_string_lossy()
        .into_owned()
}

pub(super) unsafe extern "C" fn on_engine_download_changed(
    context: *mut c_void,
    download: *const embedder::PhotonDownload,
) {
    if context.is_null() || download.is_null() {
        return;
    }
    let download = unsafe { &*download };
    let status = match download.status {
        0 => DownloadStatus::InProgress,
        1 => DownloadStatus::Paused,
        2 => DownloadStatus::Completed,
        3 => DownloadStatus::Canceled,
        _ => DownloadStatus::Failed,
    };
    let error = string(download.error);
    let change = DownloadChange::Changed(Download {
        id: download.id,
        url: string(download.url),
        path: PathBuf::from(string(download.path)),
        status,
        received_bytes: download.received_bytes,
        total_bytes: download.has_total_bytes.then_some(download.total_bytes),
        bytes_per_second: download
            .has_bytes_per_second
            .then_some(download.bytes_per_second),
        error: (!error.is_empty()).then_some(error),
        created_time_ms: download.created_time_ms,
    });
    let callback = unsafe { &*context.cast::<DownloadCallback>() };
    callback(change);
}

pub(super) unsafe extern "C" fn on_engine_download_removed(context: *mut c_void, id: u64) {
    if !context.is_null() {
        let callback = unsafe { &*context.cast::<DownloadCallback>() };
        callback(DownloadChange::Removed(id));
    }
}

impl EngineRuntime {
    /// Calls `on_change` with every download now, then as they change. It
    /// runs while Engine is pumped, so it must not update GPUI entities
    /// synchronously.
    pub(in crate::platform) fn on_download_change(
        &self,
        on_change: impl Fn(DownloadChange) + 'static,
    ) {
        if !self.is_available() {
            return;
        }
        let mut callback: Box<DownloadCallback> = Box::new(Box::new(on_change));
        let context: *mut c_void = (&mut *callback as *mut DownloadCallback).cast();
        // Keep the callback alive before Engine reports the existing list.
        let previous = self.download_callback.replace(Some(callback));
        #[cfg(not(feature = "ui-only"))]
        unsafe {
            embedder::photon_runtime_set_download_callbacks(
                self.runtime,
                context,
                Some(on_engine_download_changed),
                Some(on_engine_download_removed),
            );
        }
        #[cfg(feature = "ui-only")]
        let _ = (
            context,
            on_engine_download_changed,
            on_engine_download_removed,
        );
        drop(previous);
    }

    pub(in crate::platform) fn pause_download(&self, id: u64) {
        self.download_action(id, 0);
    }

    pub(in crate::platform) fn resume_download(&self, id: u64) {
        self.download_action(id, 1);
    }

    pub(in crate::platform) fn cancel_download(&self, id: u64) {
        self.download_action(id, 2);
    }

    fn download_action(&self, id: u64, action: i32) {
        #[cfg(not(feature = "ui-only"))]
        if self.is_available() {
            unsafe { embedder::photon_runtime_download_action(self.runtime, id, action) }
        }
        #[cfg(feature = "ui-only")]
        let _ = (id, action);
    }

    /// Removes finished, cancelled and failed downloads from the list.
    pub(in crate::platform) fn clear_inactive_downloads(&self) {
        #[cfg(not(feature = "ui-only"))]
        if self.is_available() {
            unsafe { embedder::photon_runtime_clear_inactive_downloads(self.runtime) }
        }
    }
}
