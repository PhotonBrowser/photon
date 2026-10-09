//! Photon macOS XPC transport for IOSurface and shared-event presentation.
//!
//! This crate owns the presentation broker protocol, its client channel, and
//! the native service entry point. Browser frame and lease policy stays in the
//! shell adapter.

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::{Channel, run_service};
