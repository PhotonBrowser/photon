//! macOS memory-pressure adapter backed by a libdispatch source and the
//! kernel's `memorystatus` level.

use block2::RcBlock;
use dispatch2::{
    DispatchObject, DispatchRetained, DispatchSource,
    dispatch_source_memorypressure_flags_t as MemoryPressureFlags,
};
use photon_core::MemoryPressure;

const VM_PRESSURE_LEVEL_WARNING: libc::c_int = 2;
const VM_PRESSURE_LEVEL_CRITICAL: libc::c_int = 4;

/// Calls `on_change` whenever the system pressure level may have changed, and
/// once on creation so callers start from the current state.
pub(crate) struct Monitor(DispatchRetained<DispatchSource>);

impl Monitor {
    pub(crate) fn new(on_change: impl Fn() + 'static) -> Self {
        // SAFETY: the memory-pressure source uses handle 0 and the documented
        // normal, warning, and critical event mask.
        let source = unsafe {
            DispatchSource::new(
                std::ptr::addr_of!(dispatch2::_dispatch_source_type_memorypressure).cast_mut(),
                0,
                (MemoryPressureFlags::DISPATCH_MEMORYPRESSURE_NORMAL.0
                    | MemoryPressureFlags::DISPATCH_MEMORYPRESSURE_WARN.0
                    | MemoryPressureFlags::DISPATCH_MEMORYPRESSURE_CRITICAL.0)
                    as usize,
                None,
            )
        };
        on_change();
        let handler = RcBlock::new(on_change);
        // SAFETY: the source copies the block before activation. The block
        // holds only the caller's callback and never retains the source.
        unsafe { source.set_event_handler_with_block(RcBlock::as_ptr(&handler)) };
        source.activate();
        Self(source)
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

/// Reads the kernel's current memory state instead of relying on coalesced
/// dispatch flags, which can retain a stale warning after pressure subsides.
pub(crate) fn current() -> MemoryPressure {
    let mut level: libc::c_int = 0;
    let mut size = std::mem::size_of::<libc::c_int>();
    // SAFETY: a read-only sysctl into a correctly sized integer.
    let status = unsafe {
        sysctlbyname(
            c"kern.memorystatus_vm_pressure_level".as_ptr(),
            (&mut level as *mut libc::c_int).cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if status != 0 {
        return MemoryPressure::Normal;
    }
    match level {
        VM_PRESSURE_LEVEL_CRITICAL => MemoryPressure::Critical,
        VM_PRESSURE_LEVEL_WARNING => MemoryPressure::Warning,
        _ => MemoryPressure::Normal,
    }
}

unsafe extern "C" {
    fn sysctlbyname(
        name: *const libc::c_char,
        oldp: *mut libc::c_void,
        oldlenp: *mut usize,
        newp: *mut libc::c_void,
        newlen: usize,
    ) -> libc::c_int;
}
