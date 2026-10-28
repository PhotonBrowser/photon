//! Ephemeral appearance feedback, separate from saved browser settings and policy.

use super::settings::Settings;
use gpui::{App, EntityId, Global};
use photon_core::{SpaceColors, SpaceId, WindowColor};

#[derive(Clone, Copy, Default, PartialEq)]
pub(super) struct AppearancePreview {
    pub owner: Option<EntityId>,
    pub tint: Option<(SpaceId, WindowColor, SpaceColors)>,
    pub transparency: Option<u8>,
    pub noise: Option<u8>,
}
impl Global for AppearancePreview {}

impl AppearancePreview {
    pub fn get(cx: &App) -> Self {
        cx.try_global::<Self>().copied().unwrap_or_default()
    }
    pub fn publish(self, cx: &mut App) {
        if Self::get(cx) != self {
            cx.set_global(self);
        }
    }
    pub fn clear(owner: EntityId, cx: &mut App) {
        if Self::get(cx).owner == Some(owner) {
            Self::default().publish(cx);
        }
    }
    fn has_colors(active: &photon_core::Space, cx: &App) -> bool {
        let colors = Self::get(cx)
            .tint
            .filter(|(id, _, _)| *id == active.id)
            .map(|(_, _, colors)| colors)
            .unwrap_or_else(|| super::theme::space_colors(active));
        !colors.colors().is_empty()
    }
    pub fn subtle_transparency() -> u8 {
        ((1.0
            - crate::platform::theme_config::theme()
                .transparency
                .subtle
                .window)
            * 100.0)
            .round() as u8
    }
    /// The window transparency in the space new windows open in.
    #[cfg(test)]
    pub fn transparency(cx: &App) -> u8 {
        Self::transparency_in(Settings::get(cx).spaces.active(), cx)
    }
    /// The window transparency in `space`, which follows the system's
    /// subtle look while the space has no colours.
    pub fn transparency_in(space: &photon_core::Space, cx: &App) -> u8 {
        let settings = Settings::get(cx);
        if !Self::has_colors(space, cx) {
            return ((1.0
                - crate::platform::theme_config::theme()
                    .transparency
                    .for_setting(settings.transparency)
                    .window)
                * 100.0)
                .round() as u8;
        }
        Self::get(cx)
            .transparency
            .or(settings.transparency_percent)
            .unwrap_or_else(|| {
                ((1.0
                    - crate::platform::theme_config::theme()
                        .transparency
                        .for_setting(settings.transparency)
                        .window)
                    * 100.0)
                    .round() as u8
            })
            .min(100)
    }
    pub fn noise(cx: &App) -> u8 {
        Self::get(cx)
            .noise
            .unwrap_or(Settings::get(cx).background_noise_intensity)
    }
}
