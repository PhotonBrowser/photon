//! Photon-owned GPUI elements.

mod titlebar;

#[cfg(feature = "engine")]
mod engine;
#[cfg(not(feature = "engine"))]
mod ui_only;

#[cfg(feature = "engine")]
pub use engine::ensure_linked;
#[cfg(not(feature = "engine"))]
pub use ui_only::ensure_linked;
