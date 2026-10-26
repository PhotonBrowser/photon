//! The shared theme picker in the space editor.

use super::theme_picker::{TintChange, theme_picker};
use gpui::IntoElement;
use photon_core::{SpaceColors, SpaceId, WindowColor};

pub(super) fn color_picker(
    id: &'static str,
    color: WindowColor,
    colors: SpaceColors,
    space: Option<SpaceId>,
    on_change: TintChange,
) -> impl IntoElement {
    theme_picker(id, color, colors, space, on_change)
}
