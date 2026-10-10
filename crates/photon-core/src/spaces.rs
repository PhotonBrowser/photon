//! Spaces, as in Arc: separate sets of tabs, each with its own name, window
//! colour and pinned tabs. Favourites are shared by every space.

use serde::{Deserialize, Serialize};

use super::new_tab::Shortcut;
use super::settings::WindowColor;

/// A space's identity, which stays the same as spaces are added and removed.
pub type SpaceId = u64;

/// One space.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Space {
    pub id: SpaceId,
    pub name: String,
    /// The colour the window takes while the space is shown.
    pub color: WindowColor,
    /// Whether that colour fades into a neighbouring one.
    pub gradient: bool,
    /// Tabs pinned in this space, at the addresses they were pinned at,
    /// opened again at launch.
    pub pinned_tabs: Vec<Shortcut>,
}

impl Default for Space {
    fn default() -> Self {
        Self {
            id: 1,
            name: default_name(1),
            color: WindowColor::default(),
            gradient: false,
            pinned_tabs: Vec::new(),
        }
    }
}

/// The name a new space is given: "Space 2" for the second.
fn default_name(number: usize) -> String {
    format!("Space {number}")
}

/// Every space and the one shown. There is always at least one.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Spaces {
    spaces: Vec<Space>,
    active: SpaceId,
}

impl Default for Spaces {
    fn default() -> Self {
        Self {
            spaces: vec![Space::default()],
            active: Space::default().id,
        }
    }
}

impl Spaces {
    pub fn all(&self) -> &[Space] {
        if self.spaces.is_empty() {
            // A profile with no spaces still has the default one.
            return std::slice::from_ref(fallback());
        }
        &self.spaces
    }

    /// The space shown, or the first if the saved one is gone.
    pub fn active(&self) -> &Space {
        let spaces = self.all();
        spaces
            .iter()
            .find(|space| space.id == self.active)
            .unwrap_or(&spaces[0])
    }

    pub fn get(&self, id: SpaceId) -> Option<&Space> {
        self.all().iter().find(|space| space.id == id)
    }

    pub fn get_mut(&mut self, id: SpaceId) -> Option<&mut Space> {
        self.ensure_one();
        self.spaces.iter_mut().find(|space| space.id == id)
    }

    /// Shows the space `id`, if there is one.
    pub fn activate(&mut self, id: SpaceId) -> bool {
        if self.get(id).is_none() || self.active().id == id {
            return false;
        }
        self.active = id;
        true
    }

    /// The space `offset` places from the one shown, stopping at the ends.
    pub fn neighbour(&self, offset: isize) -> Option<SpaceId> {
        let spaces = self.all();
        let index = spaces
            .iter()
            .position(|space| space.id == self.active().id)?;
        let next = index.checked_add_signed(offset)?;
        spaces.get(next).map(|space| space.id)
    }

    /// Adds a space after the others, named for its place unless `name` is
    /// given, and returns it.
    pub fn add(&mut self, name: Option<String>, color: WindowColor, gradient: bool) -> SpaceId {
        self.ensure_one();
        let id = self.spaces.iter().map(|space| space.id).max().unwrap_or(0) + 1;
        let name = name
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| default_name(self.spaces.len() + 1));
        self.spaces.push(Space {
            id,
            name,
            color,
            gradient,
            pinned_tabs: Vec::new(),
        });
        id
    }

    /// Removes the space `id`, unless it is the last. A removed space that
    /// was shown gives way to its neighbour.
    pub fn remove(&mut self, id: SpaceId) -> bool {
        self.ensure_one();
        if self.spaces.len() < 2 {
            return false;
        }
        let Some(index) = self.spaces.iter().position(|space| space.id == id) else {
            return false;
        };
        self.spaces.remove(index);
        if self.active == id {
            self.active = self.spaces[index.min(self.spaces.len() - 1)].id;
        }
        true
    }

    /// Returns every space's colour to the system grey, keeping the spaces,
    /// their names and their pinned tabs.
    pub fn reset_colors(&mut self) {
        for space in &mut self.spaces {
            space.color = WindowColor::default();
            space.gradient = false;
        }
    }

    fn ensure_one(&mut self) {
        if self.spaces.is_empty() {
            self.spaces.push(Space::default());
        }
    }
}

fn fallback() -> &'static Space {
    static FALLBACK: std::sync::OnceLock<Space> = std::sync::OnceLock::new();
    FALLBACK.get_or_init(Space::default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn always_has_a_space() {
        let spaces = Spaces {
            spaces: Vec::new(),
            active: 7,
        };
        assert_eq!(spaces.all().len(), 1);
        assert_eq!(spaces.active().name, "Space 1");
    }

    #[test]
    fn adds_switches_and_removes_spaces() {
        let mut spaces = Spaces::default();
        let work = spaces.add(Some("Work".to_owned()), WindowColor::Blue, false);
        let third = spaces.add(None, WindowColor::Green, true);
        assert_eq!(spaces.get(third).unwrap().name, "Space 3");
        assert_eq!(spaces.neighbour(1), Some(work));
        assert_eq!(spaces.neighbour(-1), None);

        assert!(spaces.activate(work));
        assert!(!spaces.activate(work));
        assert!(spaces.remove(work));
        assert_eq!(spaces.active().id, third);
        assert!(spaces.remove(third));
        assert!(!spaces.remove(spaces.active().id));
    }

    #[test]
    fn resetting_colours_keeps_the_spaces() {
        let mut spaces = Spaces::default();
        spaces.add(Some("Work".to_owned()), WindowColor::Blue, true);
        spaces.reset_colors();
        assert_eq!(spaces.all().len(), 2);
        assert!(
            spaces
                .all()
                .iter()
                .all(|space| space.color == WindowColor::System)
        );
    }
}
