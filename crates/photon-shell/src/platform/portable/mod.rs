//! Portable adapters shared by targets without native equivalents (currently
//! Linux and Windows): CPU-frame page presentation, display metadata and
//! system-service fallbacks.

pub(in crate::platform) mod display;
pub(crate) mod memory_pressure;
pub(in crate::platform) mod presentation;
pub(in crate::platform) mod renderer;
