//! Target page-presentation backend used by the shared Engine adapter.

#[cfg(target_os = "macos")]
pub(in crate::platform) use super::macos::presentation::*;

#[cfg(any(target_os = "linux", target_os = "windows"))]
pub(in crate::platform) use super::cpu_presentation::*;
