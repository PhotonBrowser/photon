//! Target display metadata used by the shared Engine adapter.

#[cfg(target_os = "macos")]
pub(in crate::platform) use super::macos::display::*;

#[cfg(not(target_os = "macos"))]
pub(in crate::platform) use super::portable::display::*;
