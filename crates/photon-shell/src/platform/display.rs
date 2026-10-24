//! Target display metadata used by the shared Engine adapter.

#[cfg(target_os = "macos")]
pub(in crate::platform) use super::macos::display::*;

#[cfg(target_os = "linux")]
pub(in crate::platform) use super::portable_display::*;

#[cfg(target_os = "windows")]
pub(in crate::platform) use super::portable_display::*;
