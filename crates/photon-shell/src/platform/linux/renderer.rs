//! Linux page renderer using GPUI's cross-platform image upload path.
//!
//! The Engine publishes CPU frames here, which GPUI uploads and draws. Native
//! composition, like the macOS Core Animation layer, would replace these with
//! a `PagePresenter` and `page_surface` of this platform's own, and with an
//! Engine surface import in place of `cpu_presentation`.

pub(in crate::platform) use super::super::cpu_presentation::PresentedSurface;
pub(in crate::platform) use super::super::cpu_renderer::{
    PagePresenter, page_surface, replace_surface,
};
