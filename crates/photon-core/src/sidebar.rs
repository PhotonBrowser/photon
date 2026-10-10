//! What the sidebar holds besides tabs, favourite sites, and how wide it is.

use serde::{Deserialize, Serialize};

use super::new_tab::{Shortcut, site_name};

/// How many favourites the sidebar holds.
pub const MAX_FAVOURITES: usize = 12;

/// The sidebar's width when first shown, in logical pixels.
pub const SIDEBAR_DEFAULT_WIDTH: u32 = 240;
/// The narrowest and widest the sidebar can be dragged.
pub const SIDEBAR_MIN_WIDTH: u32 = 200;
pub const SIDEBAR_MAX_WIDTH: u32 = 420;
/// Dragging the sidebar's edge narrower than this hides it.
pub const SIDEBAR_SNAP_WIDTH: u32 = 140;

/// What dragging the sidebar's edge to a point does.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SidebarResize {
    /// Shows the sidebar this wide.
    Width(u32),
    /// Hides the sidebar, keeping its width for when it is shown again.
    Hide,
}

impl SidebarResize {
    /// The edge dragged to `x` pixels from the window's left edge.
    pub fn to(x: f32) -> Self {
        if x < SIDEBAR_SNAP_WIDTH as f32 {
            Self::Hide
        } else {
            Self::Width((x.round() as u32).clamp(SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH))
        }
    }
}

/// The sidebar's favourite sites and width.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct SidebarSettings {
    /// Sites kept at the top of the sidebar, in this order.
    pub favourites: Vec<Shortcut>,
    /// How wide the sidebar is, in logical pixels.
    pub width: u32,
}

impl Default for SidebarSettings {
    fn default() -> Self {
        Self {
            favourites: Vec::new(),
            width: SIDEBAR_DEFAULT_WIDTH,
        }
    }
}

impl SidebarSettings {
    /// The width to draw, within the allowed range even if the saved one is
    /// not.
    pub fn width(&self) -> u32 {
        self.width.clamp(SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH)
    }

    /// The favourite for `url`'s site, if there is one.
    pub fn favourite_for(&self, url: &str) -> Option<&Shortcut> {
        let site = site_name(url);
        self.favourites
            .iter()
            .find(|favourite| site_name(&favourite.url) == site)
    }

    /// Adds a site to the favourites, unless its site is already there or the
    /// favourites are full. Returns whether it was added.
    pub fn add_favourite(&mut self, favourite: Shortcut) -> bool {
        if self.favourites.len() >= MAX_FAVOURITES || self.favourite_for(&favourite.url).is_some() {
            return false;
        }
        self.favourites.push(favourite);
        true
    }

    /// Removes the favourite for `url`'s site.
    pub fn remove_favourite(&mut self, url: &str) {
        let site = site_name(url);
        self.favourites
            .retain(|favourite| site_name(&favourite.url) != site);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shortcut(url: &str) -> Shortcut {
        Shortcut {
            title: String::new(),
            url: url.to_owned(),
        }
    }

    #[test]
    fn keeps_one_favourite_per_site() {
        let mut sidebar = SidebarSettings::default();
        assert!(sidebar.add_favourite(shortcut("https://github.com/")));
        assert!(!sidebar.add_favourite(shortcut("https://www.github.com/photon")));
        assert_eq!(sidebar.favourites.len(), 1);
        assert!(sidebar.favourite_for("https://github.com/x").is_some());
        sidebar.remove_favourite("https://github.com/anything");
        assert!(sidebar.favourites.is_empty());
    }

    #[test]
    fn holds_at_most_the_limit() {
        let mut sidebar = SidebarSettings::default();
        for index in 0..20 {
            sidebar.add_favourite(shortcut(&format!("https://{index}.example/")));
        }
        assert_eq!(sidebar.favourites.len(), MAX_FAVOURITES);
    }

    #[test]
    fn resizes_within_limits_and_snaps_shut() {
        assert_eq!(SidebarResize::to(300.4), SidebarResize::Width(300));
        assert_eq!(
            SidebarResize::to(160.0),
            SidebarResize::Width(SIDEBAR_MIN_WIDTH)
        );
        assert_eq!(
            SidebarResize::to(900.0),
            SidebarResize::Width(SIDEBAR_MAX_WIDTH)
        );
        assert_eq!(SidebarResize::to(80.0), SidebarResize::Hide);
    }

    #[test]
    fn keeps_a_saved_width_in_range() {
        let sidebar = SidebarSettings {
            width: 5,
            ..SidebarSettings::default()
        };
        assert_eq!(sidebar.width(), SIDEBAR_MIN_WIDTH);
        assert_eq!(SidebarSettings::default().width(), SIDEBAR_DEFAULT_WIDTH);
    }
}
