//! The system reduced-motion preference, for shell animations.

use std::ffi::c_void;

use super::ffi::embedder;

type OnChange = Box<dyn Fn(bool)>;

/// Reports the system reduced-motion preference until dropped.
pub(super) struct ReducedMotionObserver {
    observer: *mut c_void,
    _on_change: Box<OnChange>,
}

impl ReducedMotionObserver {
    /// `on_change(reduce_motion)` runs on the main thread, once immediately
    /// with the current preference. It must not update GPUI entities
    /// synchronously.
    pub(super) fn new(on_change: impl Fn(bool) + 'static) -> Option<Self> {
        let mut on_change: Box<OnChange> = Box::new(Box::new(on_change));
        let observer = unsafe {
            embedder::photon_reduced_motion_observer_create(
                (&mut *on_change as *mut OnChange).cast(),
                Some(on_reduced_motion_changed),
            )
        };
        (!observer.is_null()).then_some(Self {
            observer,
            _on_change: on_change,
        })
    }
}

impl Drop for ReducedMotionObserver {
    fn drop(&mut self) {
        unsafe { embedder::photon_reduced_motion_observer_destroy(self.observer) };
    }
}

unsafe extern "C" fn on_reduced_motion_changed(context: *mut c_void, reduce_motion: bool) {
    let on_change = unsafe { &*context.cast::<OnChange>() };
    on_change(reduce_motion);
}
