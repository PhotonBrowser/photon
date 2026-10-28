//! Spaces, as in Arc: separate sets of tabs, each with its own name, window
//! colour, pinned tabs and favourites.

use serde::{Deserialize, Serialize};

use super::new_tab::Shortcut;
use super::settings::WindowColor;
use super::workspace::WorkspaceLayout;

/// A space's identity, which stays the same as spaces are added and removed.
pub type SpaceId = u64;
/// How many favourites a space holds.
pub const MAX_FAVOURITES: usize = 12;

/// A custom window tint, stored in bounded integer units for stable profiles.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(default)]
pub struct SpaceTint {
    /// Hue in degrees; resolution wraps values into one turn.
    pub hue: u16,
    pub saturation: u8,
    pub lightness: u8,
}

impl Default for SpaceTint {
    fn default() -> Self {
        Self {
            hue: 216,
            saturation: 55,
            lightness: 65,
        }
    }
}

/// Maximum independent colours in a space gradient.
pub const MAX_SPACE_COLORS: usize = 3;

/// A bounded palette, shared by profile storage and the native theme picker.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(from = "RawSpaceColors")]
pub struct SpaceColors {
    colors: [SpaceTint; MAX_SPACE_COLORS],
    count: u8,
    primary: u8,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawSpaceColors {
    colors: [SpaceTint; MAX_SPACE_COLORS],
    count: u8,
    primary: u8,
}

impl From<RawSpaceColors> for SpaceColors {
    fn from(raw: RawSpaceColors) -> Self {
        let count = usize::from(raw.count).min(MAX_SPACE_COLORS);
        Self::from_colors(&raw.colors[..count]).with_primary(usize::from(raw.primary))
    }
}

impl Default for SpaceColors {
    fn default() -> Self {
        Self::from_colors(&[])
    }
}

impl SpaceColors {
    /// Keep zero to three colours; an empty palette follows the default theme.
    pub fn from_colors(colors: &[SpaceTint]) -> Self {
        let count = colors.len().min(MAX_SPACE_COLORS);
        let mut result = Self {
            colors: [SpaceTint::default(); MAX_SPACE_COLORS],
            count: count as u8,
            primary: 0,
        };
        for (target, source) in result.colors.iter_mut().zip(colors.iter()) {
            *target = *source;
        }
        result
    }
    /// The selected colour's index, bounded to the stored palette.
    pub fn primary_index(&self) -> usize {
        usize::from(self.primary).min(self.colors().len().saturating_sub(1))
    }
    /// The primary colour, absent when the space follows the default theme.
    pub fn primary(&self) -> Option<SpaceTint> {
        self.colors().get(self.primary_index()).copied()
    }
    /// Select a primary colour without reordering or changing the others.
    pub fn with_primary(mut self, index: usize) -> Self {
        self.primary = index.min(self.colors().len().saturating_sub(1)) as u8;
        self
    }
    /// The active colours, or an empty slice for the default theme.
    pub fn colors(&self) -> &[SpaceTint] {
        &self.colors[..usize::from(self.count).min(MAX_SPACE_COLORS)]
    }
}

/// One space.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Space {
    pub id: SpaceId,
    pub name: String,
    /// The colour the window takes while the space is shown.
    pub color: WindowColor,
    /// Fine-grained colour selected in the theme picker; absent for legacy presets.
    pub tint: Option<SpaceTint>,
    /// Independent colours selected in the theme picker; absent in legacy profiles.
    pub colors: Option<SpaceColors>,
    /// Whether that colour fades into a neighbouring one.
    pub gradient: bool,
    /// Tabs pinned in this space, at the addresses they were pinned at,
    /// opened again at launch.
    pub pinned_tabs: Vec<Shortcut>,
    /// Sites kept at the top of this space's sidebar.
    pub favourites: Vec<Shortcut>,
    /// Whether the empty favourites drop hint was dismissed in this space.
    pub favourites_hint_dismissed: bool,
    /// Legacy copied layout data saved before spaces selected named presets.
    pub layout_override: Option<WorkspaceLayout>,
    /// A named profile preset selected for this space.
    pub preset_override_id: Option<String>,
}

impl Default for Space {
    fn default() -> Self {
        Self {
            id: 1,
            name: default_name(1),
            color: WindowColor::default(),
            tint: None,
            colors: None,
            gradient: false,
            pinned_tabs: Vec::new(),
            favourites: Vec::new(),
            favourites_hint_dismissed: false,
            layout_override: None,
            preset_override_id: None,
        }
    }
}

impl Space {
    /// Set the authoritative palette while retaining legacy tint metadata.
    pub fn set_colors(&mut self, colors: SpaceColors) {
        self.tint = colors.primary();
        if self.tint.is_none() {
            self.color = WindowColor::System;
        }
        self.gradient = colors.colors().len() > 1;
        self.colors = Some(colors);
    }

    /// The favourite at exactly `url`, if there is one in this space. A site
    /// can be favourited more than once, at different pages.
    pub fn favourite_for(&self, url: &str) -> Option<&Shortcut> {
        self.favourites
            .iter()
            .find(|favourite| favourite.url == url)
    }

    /// Adds a page to this space's favourites, unless it is already there or
    /// the favourites are full. Returns whether it was added.
    pub fn add_favourite(&mut self, favourite: Shortcut) -> bool {
        if self.favourites.len() >= MAX_FAVOURITES || self.favourite_for(&favourite.url).is_some() {
            return false;
        }
        self.favourites.push(favourite);
        true
    }

    /// Removes the favourite at `url` from this space.
    pub fn remove_favourite(&mut self, url: &str) {
        self.favourites.retain(|favourite| favourite.url != url);
    }
}

fn tolerant_color<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<WindowColor, D::Error> {
    let name = String::deserialize(deserializer)?;
    let named = serde::de::value::StrDeserializer::<D::Error>::new(&name);
    Ok(WindowColor::deserialize(named).unwrap_or_default())
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
            tint: None,
            colors: None,
            gradient,
            pinned_tabs: Vec::new(),
            favourites: Vec::new(),
            favourites_hint_dismissed: false,
            layout_override: None,
            preset_override_id: None,
        });
        id
    }

    /// Copies a saved layout into `id`, so later preset edits do not affect it.
    pub fn set_layout_override(&mut self, id: SpaceId, layout: &WorkspaceLayout) -> bool {
        if !layout.is_valid() {
            return false;
        }
        let Some(space) = self.get_mut(id) else {
            return false;
        };
        space.layout_override = Some(layout.clone());
        space.preset_override_id = None;
        true
    }

    /// Selects a named profile preset for `id`.
    pub fn set_preset_override(&mut self, id: SpaceId, preset_id: &str) -> bool {
        if preset_id.is_empty() || preset_id.len() > 64 {
            return false;
        }
        let Some(space) = self.get_mut(id) else {
            return false;
        };
        space.preset_override_id = Some(preset_id.to_owned());
        space.layout_override = None;
        true
    }

    /// Makes `id` follow the profile's default workspace layout again.
    pub fn clear_layout_override(&mut self, id: SpaceId) -> bool {
        let Some(space) = self.get_mut(id) else {
            return false;
        };
        space.layout_override = None;
        space.preset_override_id = None;
        true
    }

    /// Removes assignments to a deleted preset so those spaces follow the
    /// profile default again.
    pub fn remove_preset_override(&mut self, preset_id: &str) {
        for space in &mut self.spaces {
            if space.preset_override_id.as_deref() == Some(preset_id) {
                space.preset_override_id = None;
            }
        }
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
            space.tint = None;
            space.colors = None;
            space.gradient = false;
        }
    }

    /// Removes every space-specific layout selection so spaces follow the profile default.
    pub fn reset_layout_overrides(&mut self) {
        for space in &mut self.spaces {
            space.layout_override = None;
            space.preset_override_id = None;
        }
    }

    /// Shows the empty favourites hint again in every space.
    pub fn reset_favourites_hint_dismissals(&mut self) {
        for space in &mut self.spaces {
            space.favourites_hint_dismissed = false;
        }
    }

    /// Drops malformed per-space layouts loaded from older or edited profile data.
    pub fn validate_layout_overrides(&mut self) {
        for space in &mut self.spaces {
            if space
                .layout_override
                .as_ref()
                .is_some_and(|layout| !layout.is_valid())
            {
                space.layout_override = None;
            }
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
    fn favourites_belong_to_their_space() {
        let mut spaces = Spaces::default();
        let first = spaces.active().id;
        let second = spaces.add(None, WindowColor::System, false);
        let favourite = Shortcut {
            title: "Example".to_owned(),
            url: "https://example.com/".to_owned(),
        };

        assert!(
            spaces
                .get_mut(first)
                .unwrap()
                .add_favourite(favourite.clone())
        );
        assert!(spaces.get(second).unwrap().favourites.is_empty());
        assert!(!spaces.get_mut(first).unwrap().add_favourite(Shortcut {
            title: "Example again".to_owned(),
            url: "https://example.com/".to_owned(),
        }));
        let other_page = Shortcut {
            title: "Another page".to_owned(),
            url: "https://example.com/other".to_owned(),
        };
        assert!(
            spaces
                .get_mut(first)
                .unwrap()
                .add_favourite(other_page.clone())
        );
        spaces
            .get_mut(first)
            .unwrap()
            .remove_favourite("https://example.com/");
        assert_eq!(spaces.get(first).unwrap().favourites, [other_page]);
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

    #[test]
    fn spaces_copy_layout_overrides_and_can_follow_the_profile_default_again() {
        let mut spaces = Spaces::default();
        let id = spaces.active().id;
        let layout = WorkspaceLayout::vertical();

        assert!(spaces.set_layout_override(id, &layout));
        assert_eq!(spaces.get(id).unwrap().layout_override, Some(layout));
        assert!(spaces.clear_layout_override(id));
        assert_eq!(spaces.get(id).unwrap().layout_override, None);
        assert!(!spaces.clear_layout_override(999));
    }

    #[test]
    fn spaces_can_select_named_presets_without_storing_a_layout_copy() {
        let mut spaces = Spaces::default();
        let id = spaces.active().id;

        assert!(spaces.set_preset_override(id, "work-right"));
        assert_eq!(
            spaces.get(id).unwrap().preset_override_id.as_deref(),
            Some("work-right")
        );
        assert!(spaces.get(id).unwrap().layout_override.is_none());
        assert!(spaces.clear_layout_override(id));
        assert!(spaces.get(id).unwrap().preset_override_id.is_none());
    }

    #[test]
    fn deleting_a_named_preset_makes_assigned_spaces_follow_the_default() {
        let mut spaces = Spaces::default();
        let id = spaces.add(None, WindowColor::Blue, false);
        assert!(spaces.set_preset_override(id, "work-right"));

        spaces.remove_preset_override("work-right");

        assert!(spaces.get(id).unwrap().preset_override_id.is_none());
    }

    #[test]
    fn malformed_layout_overrides_are_cleared() {
        let mut spaces = Spaces::default();
        let id = spaces.active().id;
        let mut invalid = WorkspaceLayout::vertical();
        invalid.components[0].order = 1;
        assert!(spaces.set_layout_override(id, &WorkspaceLayout::horizontal()));
        spaces.spaces[0].layout_override = Some(invalid);

        spaces.validate_layout_overrides();
        assert_eq!(spaces.get(id).unwrap().layout_override, None);
    }

    #[test]
    fn legacy_spaces_keep_presets_and_custom_tints_round_trip() {
        let legacy: Space = serde_json::from_str(r#"{"id":4,"color":"blue"}"#).unwrap();
        assert_eq!(legacy.color, WindowColor::Blue);
        assert_eq!(legacy.tint, None);
        let tint = SpaceTint {
            hue: 278,
            saturation: 63,
            lightness: 42,
        };
        let custom = Space {
            tint: Some(tint),
            ..legacy
        };
        let saved = serde_json::to_string(&custom).unwrap();
        assert_eq!(serde_json::from_str::<Space>(&saved).unwrap(), custom);
    }

    #[test]
    fn independent_gradient_colours_survive_storage_and_respect_bounds() {
        let a = SpaceTint {
            hue: 205,
            saturation: 61,
            lightness: 42,
        };
        let b = SpaceTint {
            hue: 270,
            saturation: 48,
            lightness: 60,
        };
        let c = SpaceTint {
            hue: 150,
            saturation: 55,
            lightness: 52,
        };
        let colors = SpaceColors::from_colors(&[a, b, c, a]);
        assert_eq!(colors.colors(), &[a, b, c]);
        assert_eq!(SpaceColors::from_colors(&[]).colors().len(), 0);
        let mut space = Space::default();
        space.set_colors(colors);
        let saved = serde_json::to_string(&space).unwrap();
        let restored: Space = serde_json::from_str(&saved).unwrap();
        assert_eq!(restored.colors.unwrap().colors(), &[a, b, c]);
        assert!(restored.gradient);
        assert_eq!(restored.tint, Some(a));
        space.set_colors(SpaceColors::from_colors(&[c]));
        assert!(!space.gradient);
        assert_eq!(space.tint, Some(c));
    }

    #[test]
    fn primary_selection_is_bounded_and_survives_storage() {
        let a = SpaceTint::default();
        let b = SpaceTint { hue: 25, ..a };
        let colors = SpaceColors::from_colors(&[a, b]).with_primary(1);
        let mut space = Space::default();
        space.set_colors(colors);
        assert_eq!(space.tint, Some(b));
        assert_eq!(colors.colors(), &[a, b]);
        let restored: Space =
            serde_json::from_str(&serde_json::to_string(&space).unwrap()).unwrap();
        assert_eq!(restored.colors.unwrap().primary_index(), 1);
        assert_eq!(restored.tint, Some(b));
        assert_eq!(colors.with_primary(99).primary_index(), 1);
        assert_eq!(SpaceColors::default().with_primary(99).primary(), None);
    }

    #[test]
    fn reset_colors_clears_custom_tints() {
        let mut spaces = Spaces::default();
        let id = spaces.active().id;
        spaces
            .get_mut(id)
            .unwrap()
            .set_colors(SpaceColors::from_colors(&[
                SpaceTint::default(),
                SpaceTint {
                    hue: 120,
                    ..SpaceTint::default()
                },
            ]));
        spaces.reset_colors();
        assert_eq!(spaces.active().tint, None);
        assert_eq!(spaces.active().colors, None);
        assert_eq!(spaces.active().color, WindowColor::System);
    }
}
