//! Memory-pressure fallback for targets without a native pressure adapter.

use photon_core::MemoryPressure;

pub(crate) struct Monitor;

impl Monitor {
    pub(crate) fn new(_: impl Fn() + 'static) -> Self {
        Self
    }
}

pub(crate) fn current() -> MemoryPressure {
    MemoryPressure::Normal
}
