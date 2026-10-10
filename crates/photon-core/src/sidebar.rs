//! What the sidebar holds besides tabs: favourite sites.

use serde::{Deserialize, Serialize};

use super::new_tab::{Shortcut, site_name};

/// How many favourites the sidebar holds.
pub const MAX_FAVOURITES: usize = 12;

/// The sidebar's favourite sites.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct SidebarSettings {
    /// Sites kept at the top of the sidebar, in this order.
    pub favourites: Vec<Shortcut>,
}

impl SidebarSettings {
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
}
