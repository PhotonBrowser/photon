//! N-API composition root: GPUIX renderer plus Photon-owned native elements.

pub use gpuix_native::*;

// Keep this crate's N-API module entrypoint in the same cdylib as GPUIX's
// exported renderer. The linked Photon element contributes its static factory
// registration to GPUIX's inventory before `GpuixRenderer::init()` runs.
#[napi_derive::napi]
pub fn photon_native_addon() -> bool {
    photon_gpui::ensure_linked();
    true
}
