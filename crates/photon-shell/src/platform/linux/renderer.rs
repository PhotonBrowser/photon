//! Linux page renderer, currently on the interim CPU-frame path.
//!
//! The Engine copies each frame into CPU memory, and GPUI uploads it to the GPU
//! and draws it. It is correct but costs a full-frame copy and upload per
//! frame, so it is a bring-up path, not the intended design. Replace it with
//! DMA-BUF import: a `PagePresenter` and `page_surface` of this platform's own,
//! like the macOS Core Animation layer, and an Engine surface export in place
//! of `portable::presentation`. See `Documentation/Linux.md`.

pub(in crate::platform) use crate::platform::portable::presentation::PresentedSurface;
pub(in crate::platform) use crate::platform::portable::renderer::{
    PagePresenter, page_surface, replace_surface,
};
