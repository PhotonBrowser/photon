//! Native window occlusion and display-change notifications.

use gpui::Window;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::ffi::c_void;

use super::ffi::embedder;

type OnChange = Box<dyn Fn(bool, bool)>;

/// Reports whether the window is visible and when its display or the
/// display's parameters may have changed, until dropped.
pub(super) struct WindowObserver {
    observer: *mut c_void,
    _on_change: Box<OnChange>,
}

impl WindowObserver {
    /// `on_change(visible, display_changed)` runs on the main thread, once
    /// immediately with the current visibility. It must not update GPUI
    /// entities synchronously.
    pub(super) fn new(window: &Window, on_change: impl Fn(bool, bool) + 'static) -> Option<Self> {
        let RawWindowHandle::AppKit(handle) = HasWindowHandle::window_handle(window).ok()?.as_raw() else {
            return None;
        };
        let mut on_change: Box<OnChange> = Box::new(Box::new(on_change));
        let observer = unsafe {
            embedder::photon_window_observer_create(
                handle.ns_view.as_ptr(),
                (&mut *on_change as *mut OnChange).cast(),
                Some(on_window_changed),
            )
        };
        (!observer.is_null()).then_some(Self {
            observer,
            _on_change: on_change,
        })
    }
}

impl Drop for WindowObserver {
    fn drop(&mut self) {
        unsafe { embedder::photon_window_observer_destroy(self.observer) };
    }
}

unsafe extern "C" fn on_window_changed(context: *mut c_void, visible: bool, display_changed: bool) {
    let on_change = unsafe { &*context.cast::<OnChange>() };
    on_change(visible, display_changed);
}
