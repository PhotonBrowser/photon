//! The Engine runtime, which every tab's session shares, and the sessions:
//! see `session`.

mod callback_state;
mod callbacks;
pub(super) mod navigation_timing;
mod session;

use anyhow::Context as _;
use gpui::{AsyncApp, WeakEntity};
use photon_core::{ClearBrowsingData, PopupPolicy};
use photon_performance::PerformanceTask;
use std::{
    cell::{Cell, RefCell},
    ffi::{CStr, CString, c_void},
    path::Path,
    rc::Rc,
    sync::{
        Mutex,
        atomic::{AtomicU8, Ordering},
    },
};

use super::ffi::embedder;
use crate::platform::ui::PhotonWebView;
use callbacks::*;
use photon_core::EngineService;
pub(in crate::platform) use session::{EngineSession, RequestedWebView};

#[derive(Clone)]
pub(in crate::platform) struct UiWake {
    pub(in crate::platform) app: AsyncApp,
    pub(in crate::platform) webview: WeakEntity<PhotonWebView>,
    pub(in crate::platform) runtime: Rc<EngineRuntime>,
}

pub(in crate::platform) struct EngineRuntime {
    runtime: *mut c_void,
    reduced_motion_observer: *mut c_void,
    callbacks: Box<RuntimeCallbacks>,
    service_callback: RefCell<Option<Box<ServiceCallback>>>,
    popup_policy: AtomicU8,
    performance_dashboard_users: Cell<usize>,
}

impl EngineRuntime {
    /// Starts the Engine, keeping website data in `profile_dir`, or in a
    /// temporary profile when there is none.
    pub(in crate::platform) fn create(profile_dir: Option<&Path>) -> anyhow::Result<Self> {
        let helper = std::env::var("PHOTON_HELPER_DIRECTORY")
            .context("PHOTON_HELPER_DIRECTORY is not set")?;
        let helper = CString::new(helper)?;
        let profile = profile_dir
            .map(|dir| CString::new(dir.to_string_lossy().into_owned()))
            .transpose()?;
        let mut error = [0_i8; 1024];
        let runtime = unsafe {
            embedder::photon_runtime_create(
                helper.as_ptr(),
                profile
                    .as_ref()
                    .map_or(std::ptr::null(), |profile| profile.as_ptr()),
                error.as_mut_ptr(),
                error.len(),
            )
        };
        anyhow::ensure!(
            !runtime.is_null(),
            "Photon Engine runtime failed: {}",
            unsafe { CStr::from_ptr(error.as_ptr()) }.to_string_lossy()
        );
        let mut callbacks = Box::new(RuntimeCallbacks {
            views: Mutex::new(Vec::new()),
        });
        let reduced_motion_observer = unsafe {
            embedder::photon_reduced_motion_observer_create(
                runtime,
                Some(on_system_reduced_motion_changed),
            )
        };
        #[cfg(target_os = "macos")]
        unsafe {
            embedder::photon_runtime_use_system_clipboard(runtime);
        }
        unsafe {
            embedder::photon_runtime_set_native_release_drain_callback(
                runtime,
                (&mut *callbacks as *mut RuntimeCallbacks).cast(),
                Some(on_native_release_drain),
            );
        }
        Ok(Self {
            runtime,
            reduced_motion_observer,
            callbacks,
            service_callback: RefCell::default(),
            popup_policy: AtomicU8::new(PopupPolicy::Ask as u8),
            performance_dashboard_users: Cell::new(0),
        })
    }

    pub(in crate::platform) fn popup_policy(&self) -> PopupPolicy {
        match self.popup_policy.load(Ordering::Relaxed) {
            value if value == PopupPolicy::Allow as u8 => PopupPolicy::Allow,
            value if value == PopupPolicy::Block as u8 => PopupPolicy::Block,
            _ => PopupPolicy::Ask,
        }
    }

    pub(in crate::platform) fn set_popup_policy(&self, policy: PopupPolicy) {
        self.popup_policy.store(policy as u8, Ordering::Relaxed);
    }

    pub(in crate::platform) fn set_elastic_scroll(&self, enabled: bool) {
        unsafe { embedder::photon_runtime_set_elastic_scroll_enabled(self.runtime, enabled) }
    }

    pub(in crate::platform) fn performance_tasks(&self) -> Vec<PerformanceTask> {
        let views = {
            let mut views = self.callbacks.views.lock().unwrap();
            views.retain(|view| view.strong_count() > 0);
            views
                .iter()
                .filter_map(std::sync::Weak::upgrade)
                .collect::<Vec<_>>()
        };
        views.iter().map(|view| view.performance_task()).collect()
    }

    pub(in crate::platform) fn acquire_performance_dashboard(
        self: &Rc<Self>,
    ) -> PerformanceDashboardLease {
        let users = self.performance_dashboard_users.get();
        self.performance_dashboard_users.set(users + 1);
        if users == 0 {
            self.set_performance_dashboard_enabled(true);
        }
        PerformanceDashboardLease {
            runtime: self.clone(),
        }
    }

    fn release_performance_dashboard(&self) {
        let users = self.performance_dashboard_users.get();
        debug_assert!(users > 0);
        if users <= 1 {
            self.performance_dashboard_users.set(0);
            self.set_performance_dashboard_enabled(false);
        } else {
            self.performance_dashboard_users.set(users - 1);
        }
    }

    fn set_performance_dashboard_enabled(&self, enabled: bool) {
        let views = self
            .callbacks
            .views
            .lock()
            .unwrap()
            .iter()
            .filter_map(std::sync::Weak::upgrade)
            .collect::<Vec<_>>();
        for view in views {
            view.set_dashboard_enabled(enabled);
        }
    }

    pub(in crate::platform) fn performance_dashboard_active(&self) -> bool {
        self.performance_dashboard_users.get() > 0
    }
}

/// Keeps Engine sampling enabled while at least one performance page is open.
pub(in crate::platform) struct PerformanceDashboardLease {
    runtime: Rc<EngineRuntime>,
}

impl Drop for PerformanceDashboardLease {
    fn drop(&mut self) {
        self.runtime.release_performance_dashboard();
    }
}

impl EngineRuntime {
    /// Calls `on_change(service, restarted)` on the main thread when an Engine
    /// service stops (`false`) and once it is running again (`true`). It must
    /// not update GPUI entities synchronously.
    pub(in crate::platform) fn on_service_change(
        &self,
        on_change: impl Fn(EngineService, bool) + 'static,
    ) {
        let mut callback: Box<ServiceCallback> = Box::new(Box::new(on_change));
        unsafe {
            embedder::photon_runtime_set_service_callback(
                self.runtime,
                (&mut *callback as *mut ServiceCallback).cast(),
                Some(on_engine_service),
            );
        }
        self.service_callback.replace(Some(callback));
    }
}

impl EngineRuntime {
    /// Deletes the website data `request` asks for, then calls `done` on the
    /// main thread. `done` must not update GPUI entities synchronously.
    pub(in crate::platform) fn clear_browsing_data(
        &self,
        request: &ClearBrowsingData,
        done: impl FnOnce() + 'static,
    ) {
        if !request.touches_engine() {
            done();
            return;
        }
        let done: Box<BrowsingDataCleared> = Box::new(Box::new(done));
        unsafe {
            embedder::photon_runtime_clear_browsing_data(
                self.runtime,
                i64::try_from(request.since).unwrap_or(i64::MAX),
                request.cache,
                request.site_data,
                Box::into_raw(done).cast(),
                Some(on_engine_browsing_data_cleared),
            );
        }
    }
}

impl Drop for EngineRuntime {
    fn drop(&mut self) {
        unsafe {
            if !self.runtime.is_null() {
                embedder::photon_runtime_set_native_release_drain_callback(
                    self.runtime,
                    std::ptr::null_mut(),
                    None,
                );
            }
            if !self.reduced_motion_observer.is_null() {
                embedder::photon_reduced_motion_observer_destroy(self.reduced_motion_observer);
                self.reduced_motion_observer = std::ptr::null_mut();
            }
            if !self.runtime.is_null() {
                embedder::photon_runtime_destroy(self.runtime);
                self.runtime = std::ptr::null_mut();
            }
        }
    }
}

/// A change to a page's zoom level.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::platform) enum ZoomStep {
    In,
    Out,
    Reset,
}

unsafe extern "C" fn on_system_reduced_motion_changed(runtime: *mut c_void, reduce_motion: bool) {
    if !runtime.is_null() {
        unsafe {
            embedder::photon_runtime_set_system_reduced_motion_preference(runtime, reduce_motion)
        };
    }
}
