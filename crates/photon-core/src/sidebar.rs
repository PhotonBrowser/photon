//! Sidebar sizing and legacy profile-wide favourites.

use serde::{Deserialize, Serialize};

use super::new_tab::Shortcut;

/// The sidebar's width when first shown, in logical pixels. It is also the
/// narrowest it can be with its labels, since its content needs this room:
/// dragging its edge narrower collapses it to icons, then hides it.
pub const SIDEBAR_DEFAULT_WIDTH: u32 = 240;
/// The widest the sidebar can be dragged.
pub const SIDEBAR_MAX_WIDTH: u32 = 420;
/// The sidebar's width when collapsed to a column of icons.
pub const SIDEBAR_COLLAPSED_WIDTH: u32 = 44;

/// What dragging the sidebar's edge to a point does.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SidebarResize {
    /// Shows the sidebar this wide, with its labels.
    Width(u32),
    /// Collapses the sidebar to icons, keeping its width for when it expands.
    Collapse,
    /// Hides the sidebar, keeping its width for when it is shown again.
    Hide,
}

impl SidebarResize {
    /// The edge dragged to `x` pixels from the window's side. Between two
    /// widths it snaps to the nearer one.
    pub fn to(x: f32) -> Self {
        let (collapsed, expanded) = (SIDEBAR_COLLAPSED_WIDTH as f32, SIDEBAR_DEFAULT_WIDTH as f32);
        if x < collapsed / 2.0 {
            Self::Hide
        } else if x < (collapsed + expanded) / 2.0 {
            Self::Collapse
        } else {
            Self::Width((x.round() as u32).clamp(SIDEBAR_DEFAULT_WIDTH, SIDEBAR_MAX_WIDTH))
        }
    }
}

/// The sidebar's width and pre-space favourites saved by older profiles.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct SidebarSettings {
    /// Favourites saved before they belonged to a space. The shell migrates
    /// these into the active space when loading an older profile.
    #[serde(skip_serializing)]
    pub favourites: Vec<Shortcut>,
    /// How wide the sidebar is, in logical pixels.
    pub width: u32,
    /// Whether the sidebar shows only icons, so new windows open it that way.
    pub collapsed: bool,
}

impl Default for SidebarSettings {
    fn default() -> Self {
        Self {
            favourites: Vec::new(),
            width: SIDEBAR_DEFAULT_WIDTH,
            collapsed: false,
        }
    }
}

impl SidebarSettings {
    /// The width to draw, within the allowed range even if the saved one is
    /// not.
    pub fn width(&self) -> u32 {
        self.width.clamp(SIDEBAR_DEFAULT_WIDTH, SIDEBAR_MAX_WIDTH)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widens_up_to_the_limit_then_collapses_and_hides_below_the_default() {
        assert_eq!(SidebarResize::to(300.4), SidebarResize::Width(300));
        assert_eq!(
            SidebarResize::to(240.0),
            SidebarResize::Width(SIDEBAR_DEFAULT_WIDTH)
        );
        assert_eq!(
            SidebarResize::to(900.0),
            SidebarResize::Width(SIDEBAR_MAX_WIDTH)
        );
        // Nearer the default width than the icons, it holds the default.
        assert_eq!(
            SidebarResize::to(160.0),
            SidebarResize::Width(SIDEBAR_DEFAULT_WIDTH)
        );
        assert_eq!(SidebarResize::to(140.0), SidebarResize::Collapse);
        assert_eq!(SidebarResize::to(30.0), SidebarResize::Collapse);
        assert_eq!(SidebarResize::to(20.0), SidebarResize::Hide);
    }

    #[test]
    fn keeps_a_saved_width_in_range() {
        let sidebar = SidebarSettings {
            width: 5,
            ..SidebarSettings::default()
        };
        assert_eq!(sidebar.width(), SIDEBAR_DEFAULT_WIDTH);
        assert_eq!(SidebarSettings::default().width(), SIDEBAR_DEFAULT_WIDTH);
    }
}
