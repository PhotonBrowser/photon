//! The Engine runtime, which every tab's session shares, and the sessions:
//! see `session`.

mod callback_state;
mod callbacks;
pub(super) mod navigation_timing;
mod session;

#[cfg(not(feature = "ui-only"))]
use anyhow::Context as _;
use gpui::{AsyncApp, WeakEntity};
use photon_core::{ClearBrowsingData, PopupPolicy};
use photon_performance::PerformanceTask;
#[cfg(not(feature = "ui-only"))]
use std::cell::RefCell;
use std::{
    cell::Cell,
    ffi::{CStr, c_void},
    rc::Rc,
    sync::{
        Mutex,
        atomic::{AtomicU8, Ordering},
    },
};
#[cfg(not(feature = "ui-only"))]
use std::{ffi::CString, path::Path};

use super::ffi::embedder;
use crate::platform::ui::PhotonWebView;
use callbacks::*;
#[cfg(not(feature = "ui-only"))]
use photon_core::EngineService;
pub(in crate::platform) use session::{EngineSession, RequestedWebView};

#[derive(Clone, Debug)]
pub(crate) struct RegisteredEngineProcess {
    pub(crate) pid: u32,
    pub(crate) role: String,
    pub(crate) start_time_in_microseconds: Option<u64>,
    pub(crate) parent_pid: Option<u32>,
    pub(crate) parent_start_time_in_microseconds: Option<u64>,
}

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
    #[cfg(not(feature = "ui-only"))]
    service_callback: RefCell<Option<Box<ServiceCallback>>>,
    popup_policy: AtomicU8,
    performance_dashboard_users: Cell<usize>,
}

impl EngineRuntime {
    /// Starts the Engine, keeping website data in `profile_dir`, or in a
    /// temporary profile when there is none.
    #[cfg(not(feature = "ui-only"))]
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
            #[cfg(not(feature = "ui-only"))]
            service_callback: RefCell::default(),
            popup_policy: AtomicU8::new(PopupPolicy::Ask as u8),
            performance_dashboard_users: Cell::new(0),
        })
    }

    /// Creates the shell runtime used by UI-only builds, with no Engine handles.
    #[cfg(feature = "ui-only")]
    pub(in crate::platform) fn without_engine() -> Self {
        Self {
            runtime: std::ptr::null_mut(),
            reduced_motion_observer: std::ptr::null_mut(),
            callbacks: Box::new(RuntimeCallbacks {
                views: Mutex::new(Vec::new()),
            }),
            popup_policy: AtomicU8::new(PopupPolicy::Ask as u8),
            performance_dashboard_users: Cell::new(0),
        }
    }

    pub(in crate::platform) fn is_available(&self) -> bool {
        !self.runtime.is_null()
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
        if !self.is_available() {
            return;
        }
        unsafe { embedder::photon_runtime_set_elastic_scroll_enabled(self.runtime, enabled) }
    }

    pub(in crate::platform) fn performance_tasks(&self) -> Vec<PerformanceTask> {
        if !self.is_available() {
            return Vec::new();
        }
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
        if !self.is_available() {
            return;
        }
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
    #[cfg(not(feature = "ui-only"))]
    pub(in crate::platform) fn on_service_change(
        &self,
        on_change: impl Fn(EngineService, bool) + 'static,
    ) {
        if !self.is_available() {
            return;
        }
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

    pub(in crate::platform) fn registered_processes(&self) -> Vec<RegisteredEngineProcess> {
        if !self.is_available() {
            return Vec::new();
        }
        unsafe extern "C" fn collect(
            context: *mut c_void,
            pid: i32,
            role: *const std::ffi::c_char,
            has_start_time: bool,
            start_time_in_microseconds: u64,
            has_parent_pid: bool,
            parent_pid: i32,
            has_parent_start_time: bool,
            parent_start_time_in_microseconds: u64,
        ) {
            if context.is_null() || pid <= 0 || role.is_null() {
                return;
            }
            // SAFETY: the embedder invokes this callback synchronously with the
            // Vec pointer supplied below and a process-role string that stays
            // valid for the duration of the callback.
            let processes = unsafe { &mut *context.cast::<Vec<RegisteredEngineProcess>>() };
            let role = unsafe { CStr::from_ptr(role) }
                .to_string_lossy()
                .into_owned();
            processes.push(RegisteredEngineProcess {
                pid: pid as u32,
                role,
                start_time_in_microseconds: has_start_time.then_some(start_time_in_microseconds),
                parent_pid: has_parent_pid.then_some(parent_pid as u32),
                parent_start_time_in_microseconds: has_parent_start_time
                    .then_some(parent_start_time_in_microseconds),
            });
        }

        let mut processes = Vec::new();
        unsafe {
            embedder::photon_runtime_list_registered_processes(
                self.runtime,
                (&mut processes as *mut Vec<RegisteredEngineProcess>).cast(),
                Some(collect),
            );
        }
        processes
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
        if !self.is_available() || !request.touches_engine() {
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

#[cfg(not(feature = "ui-only"))]
unsafe extern "C" fn on_system_reduced_motion_changed(runtime: *mut c_void, reduce_motion: bool) {
    if !runtime.is_null() {
        unsafe {
            embedder::photon_runtime_set_system_reduced_motion_preference(runtime, reduce_motion)
        };
    }
}
