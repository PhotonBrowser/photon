//! Serializable browser workspace layouts and profile presets.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::TabLayout;

pub const HORIZONTAL_PRESET_ID: &str = "horizontal";
pub const VERTICAL_PRESET_ID: &str = "vertical";

pub const TAB_STRIP_COMPONENT_ID: &str = "tab-strip";
/// Legacy composite IDs retained for migration of saved layouts.
pub const ADDRESS_TOOLBAR_COMPONENT_ID: &str = "address-toolbar";
pub const SIDEBAR_COMPONENT_ID: &str = "sidebar";
pub const BACK_BUTTON_COMPONENT_ID: &str = "back-button";
pub const FORWARD_BUTTON_COMPONENT_ID: &str = "forward-button";
pub const RELOAD_BUTTON_COMPONENT_ID: &str = "reload-button";
pub const ADDRESS_FIELD_COMPONENT_ID: &str = "address-field";
pub const BROWSER_MENU_BUTTON_COMPONENT_ID: &str = "browser-menu-button";
pub const SIDEBAR_TOGGLE_COMPONENT_ID: &str = "sidebar-toggle";
pub const FAVOURITES_PANEL_COMPONENT_ID: &str = "favourites-panel";
pub const VERTICAL_TABS_COMPONENT_ID: &str = "vertical-tabs";
pub const SIDEBAR_FOOTER_COMPONENT_ID: &str = "sidebar-footer";

const MAX_PRESETS: usize = 32;
const MAX_COMPONENTS_PER_LAYOUT: usize = 64;
const MAX_ID_LENGTH: usize = 64;
const MAX_PRESET_NAME_LENGTH: usize = 64;
const MAX_PREFERRED_EXTENT: u16 = 4096;

/// Which side of the window holds the vertical workspace sidebar.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SidebarSide {
    #[default]
    Left,
    Right,
}

/// A dock edge around the page. Leading and trailing follow text direction.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DockRegion {
    #[default]
    Top,
    Bottom,
    Leading,
    Trailing,
}

impl DockRegion {
    fn index(self) -> usize {
        match self {
            Self::Top => 0,
            Self::Bottom => 1,
            Self::Leading => 2,
            Self::Trailing => 3,
        }
    }
}

/// One shell component's saved position and visibility.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct LayoutComponent {
    /// Stable shell catalog ID, such as `tab-strip`.
    pub id: String,
    pub region: DockRegion,
    /// Zero-based order within `region`, including hidden components.
    pub order: u16,
    pub visible: bool,
    /// Preferred cross-axis extent in logical points, when the component is resizable.
    pub preferred_extent: Option<u16>,
}

impl Default for LayoutComponent {
    fn default() -> Self {
        Self {
            id: String::new(),
            region: DockRegion::Top,
            order: 0,
            visible: true,
            preferred_extent: None,
        }
    }
}

impl LayoutComponent {
    fn new(id: &str, region: DockRegion, order: u16, visible: bool) -> Self {
        Self {
            id: id.to_owned(),
            region,
            order,
            visible,
            preferred_extent: None,
        }
    }
}

/// Component placements saved in a profile preset.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct WorkspaceLayout {
    pub components: Vec<LayoutComponent>,
}

impl WorkspaceLayout {
    /// The default horizontal composition, expressed as catalog components.
    pub fn horizontal() -> Self {
        Self::from_catalog_components([
            (TAB_STRIP_COMPONENT_ID, DockRegion::Top, true),
            (BACK_BUTTON_COMPONENT_ID, DockRegion::Top, true),
            (FORWARD_BUTTON_COMPONENT_ID, DockRegion::Top, true),
            (RELOAD_BUTTON_COMPONENT_ID, DockRegion::Top, true),
            (ADDRESS_FIELD_COMPONENT_ID, DockRegion::Top, true),
            (BROWSER_MENU_BUTTON_COMPONENT_ID, DockRegion::Top, true),
            (SIDEBAR_TOGGLE_COMPONENT_ID, DockRegion::Leading, false),
            (FAVOURITES_PANEL_COMPONENT_ID, DockRegion::Leading, false),
            (VERTICAL_TABS_COMPONENT_ID, DockRegion::Leading, false),
            (SIDEBAR_FOOTER_COMPONENT_ID, DockRegion::Leading, false),
        ])
    }

    /// The default vertical composition, expressed as catalog components.
    pub fn vertical() -> Self {
        Self::from_catalog_components([
            (TAB_STRIP_COMPONENT_ID, DockRegion::Top, false),
            (SIDEBAR_TOGGLE_COMPONENT_ID, DockRegion::Leading, true),
            (BACK_BUTTON_COMPONENT_ID, DockRegion::Leading, true),
            (FORWARD_BUTTON_COMPONENT_ID, DockRegion::Leading, true),
            (RELOAD_BUTTON_COMPONENT_ID, DockRegion::Leading, true),
            (ADDRESS_FIELD_COMPONENT_ID, DockRegion::Leading, true),
            (FAVOURITES_PANEL_COMPONENT_ID, DockRegion::Leading, true),
            (VERTICAL_TABS_COMPONENT_ID, DockRegion::Leading, true),
            (SIDEBAR_FOOTER_COMPONENT_ID, DockRegion::Leading, true),
            (BROWSER_MENU_BUTTON_COMPONENT_ID, DockRegion::Top, false),
        ])
    }

    /// A standard browser layout with its sidebar on the chosen side.
    pub fn for_tab_layout_and_side(tab_layout: TabLayout, side: SidebarSide) -> Self {
        let layout = match tab_layout {
            TabLayout::Horizontal => Self::horizontal(),
            TabLayout::Vertical => Self::vertical(),
        };
        layout.with_sidebar_side(side)
    }

    /// Returns the tab mode represented by this layout, including old saved
    /// layouts that used composite sidebar and toolbar components.
    pub fn tab_layout(&self) -> TabLayout {
        let vertical_tabs_visible = self
            .component(VERTICAL_TABS_COMPONENT_ID)
            .is_some_and(|component| component.visible);
        let horizontal_tabs_hidden = self
            .component(TAB_STRIP_COMPONENT_ID)
            .is_some_and(|component| !component.visible);
        if vertical_tabs_visible || horizontal_tabs_hidden {
            TabLayout::Vertical
        } else {
            TabLayout::Horizontal
        }
    }

    /// Returns the logical side used by the sidebar components.
    pub fn sidebar_side(&self) -> SidebarSide {
        let region = [
            SIDEBAR_TOGGLE_COMPONENT_ID,
            VERTICAL_TABS_COMPONENT_ID,
            FAVOURITES_PANEL_COMPONENT_ID,
            SIDEBAR_FOOTER_COMPONENT_ID,
            SIDEBAR_COMPONENT_ID,
        ]
        .into_iter()
        .find_map(|id| self.component(id).map(|component| component.region));
        match region {
            Some(DockRegion::Trailing) => SidebarSide::Right,
            _ => SidebarSide::Left,
        }
    }

    /// Moves all side-docked components together, preserving their order.
    pub fn with_sidebar_side(mut self, side: SidebarSide) -> Self {
        let target = match side {
            SidebarSide::Left => DockRegion::Leading,
            SidebarSide::Right => DockRegion::Trailing,
        };
        let mut side_components = self
            .components
            .iter()
            .filter(|component| {
                matches!(component.region, DockRegion::Leading | DockRegion::Trailing)
            })
            .cloned()
            .collect::<Vec<_>>();
        side_components.sort_by_key(|component| (component.region.index(), component.order));
        self.components.retain(|component| {
            !matches!(component.region, DockRegion::Leading | DockRegion::Trailing)
        });
        for (order, component) in side_components.iter_mut().enumerate() {
            component.region = target;
            component.order = order as u16;
        }
        self.components.extend(side_components);
        self
    }

    fn from_catalog_components<const N: usize>(components: [(&str, DockRegion, bool); N]) -> Self {
        let mut orders = [0u16; 4];
        let components = components
            .into_iter()
            .map(|(id, region, visible)| {
                let order = &mut orders[region.index()];
                let component = LayoutComponent::new(id, region, *order, visible);
                *order += 1;
                component
            })
            .collect();
        Self { components }
    }

    #[cfg(test)]
    fn legacy_horizontal() -> Self {
        Self {
            components: vec![
                LayoutComponent::new(TAB_STRIP_COMPONENT_ID, DockRegion::Top, 0, true),
                LayoutComponent::new(ADDRESS_TOOLBAR_COMPONENT_ID, DockRegion::Top, 1, true),
                LayoutComponent::new(SIDEBAR_COMPONENT_ID, DockRegion::Leading, 0, false),
            ],
        }
    }

    #[cfg(test)]
    fn legacy_vertical() -> Self {
        Self {
            components: vec![
                LayoutComponent::new(TAB_STRIP_COMPONENT_ID, DockRegion::Top, 0, false),
                LayoutComponent::new(ADDRESS_TOOLBAR_COMPONENT_ID, DockRegion::Top, 1, false),
                LayoutComponent::new(SIDEBAR_COMPONENT_ID, DockRegion::Leading, 0, true),
            ],
        }
    }

    pub fn component(&self, id: &str) -> Option<&LayoutComponent> {
        self.components.iter().find(|component| component.id == id)
    }

    /// Adds a component that is new to this saved layout, appending it to the
    /// chosen region. This lets newer shell catalogs extend older presets.
    pub fn add_component(&mut self, id: &str, region: DockRegion, visible: bool) -> bool {
        if !self.is_valid()
            || self.components.len() >= MAX_COMPONENTS_PER_LAYOUT
            || !is_stable_id(id)
            || self.component(id).is_some()
        {
            return false;
        }
        let order = self
            .components
            .iter()
            .filter(|component| component.region == region)
            .count() as u16;
        self.components
            .push(LayoutComponent::new(id, region, order, visible));
        true
    }

    /// Changes a component's visibility. Shell-specific capability groups
    /// decide which combinations are valid for a particular component catalog.
    pub fn set_component_visible(&mut self, id: &str, visible: bool) -> bool {
        let Some(index) = self
            .components
            .iter()
            .position(|component| component.id == id)
        else {
            return false;
        };
        if self.components[index].visible == visible {
            return false;
        }
        self.components[index].visible = visible;
        true
    }

    /// Moves a component to a dock region and position. The index is clamped
    /// to the number of components already in that region.
    pub fn move_component(&mut self, id: &str, region: DockRegion, index: usize) -> bool {
        if !self.components.iter().any(|component| component.id == id) {
            return false;
        }
        let mut groups: [Vec<LayoutComponent>; 4] = std::array::from_fn(|_| Vec::new());
        for component in self.components.drain(..) {
            groups[component.region.index()].push(component);
        }
        for group in &mut groups {
            group.sort_by_key(|component| component.order);
        }
        let source_region = groups
            .iter()
            .position(|group| group.iter().any(|component| component.id == id));
        let Some(source_region) = source_region else {
            return false;
        };
        let source_position = groups[source_region]
            .iter()
            .position(|component| component.id == id)
            .expect("component was found in its source region");
        let component = groups[source_region].remove(source_position);
        let target = &mut groups[region.index()];
        let target_index = index.min(target.len());
        let changed = source_region != region.index() || source_position != target_index;
        if !changed {
            groups[region.index()].insert(target_index, component);
            self.components = groups.into_iter().flatten().collect();
            return false;
        }
        target.insert(target_index, component);
        for (region_index, group) in groups.iter_mut().enumerate() {
            for (order, component) in group.iter_mut().enumerate() {
                component.region = match region_index {
                    0 => DockRegion::Top,
                    1 => DockRegion::Bottom,
                    2 => DockRegion::Leading,
                    _ => DockRegion::Trailing,
                };
                component.order = order as u16;
            }
        }
        self.components = groups.into_iter().flatten().collect();
        true
    }

    /// Sets a component's preferred cross-axis size in logical points.
    pub fn set_preferred_extent(&mut self, id: &str, extent: Option<u16>) -> bool {
        if extent.is_some_and(|extent| extent == 0 || extent > MAX_PREFERRED_EXTENT) {
            return false;
        }
        let Some(component) = self
            .components
            .iter_mut()
            .find(|component| component.id == id)
        else {
            return false;
        };
        if component.preferred_extent == extent {
            return false;
        }
        component.preferred_extent = extent;
        true
    }

    /// Checks profile-independent structure. The shell catalog applies
    /// component-specific placement and size constraints.
    pub fn is_valid(&self) -> bool {
        if self.components.is_empty() || self.components.len() > MAX_COMPONENTS_PER_LAYOUT {
            return false;
        }

        let mut ids = HashSet::with_capacity(self.components.len());
        let mut orders: [Vec<u16>; 4] = std::array::from_fn(|_| Vec::new());
        for component in &self.components {
            if !is_stable_id(&component.id)
                || !ids.insert(component.id.as_str())
                || component
                    .preferred_extent
                    .is_some_and(|extent| extent == 0 || extent > MAX_PREFERRED_EXTENT)
            {
                return false;
            }
            orders[component.region.index()].push(component.order);
        }

        orders.iter_mut().all(|region_orders| {
            region_orders.sort_unstable();
            region_orders
                .iter()
                .enumerate()
                .all(|(expected, order)| usize::from(*order) == expected)
        })
    }
}

/// A named, reusable arrangement of browser chrome.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct WorkspacePreset {
    pub id: String,
    pub name: String,
    pub layout: WorkspaceLayout,
}

/// Profile-level presets and the preset used by spaces without an override.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct WorkspaceSettings {
    pub default_preset_id: String,
    pub presets: Vec<WorkspacePreset>,
}

impl Default for WorkspaceSettings {
    fn default() -> Self {
        Self {
            default_preset_id: VERTICAL_PRESET_ID.to_owned(),
            presets: vec![
                WorkspacePreset {
                    id: HORIZONTAL_PRESET_ID.to_owned(),
                    name: "Horizontal".to_owned(),
                    layout: WorkspaceLayout::horizontal(),
                },
                WorkspacePreset {
                    id: VERTICAL_PRESET_ID.to_owned(),
                    name: "Vertical".to_owned(),
                    layout: WorkspaceLayout::vertical(),
                },
            ],
        }
    }
}

impl WorkspaceSettings {
    /// Defaults a profile to the layout it used before workspace presets were
    /// persisted.
    pub fn for_tab_layout(tab_layout: TabLayout) -> Self {
        let mut settings = Self::default();
        settings.set_builtin_layout(tab_layout);
        settings
    }

    /// Keeps the appearance setting and profile default preset in sync.
    pub fn set_builtin_layout(&mut self, tab_layout: TabLayout) {
        let preferred_id = match tab_layout {
            TabLayout::Horizontal => HORIZONTAL_PRESET_ID,
            TabLayout::Vertical => VERTICAL_PRESET_ID,
        };
        self.default_preset_id = self
            .presets
            .iter()
            .find(|preset| preset.id == preferred_id && preset.layout.tab_layout() == tab_layout)
            .or_else(|| {
                self.presets
                    .iter()
                    .find(|preset| preset.layout.tab_layout() == tab_layout)
            })
            .or_else(|| self.presets.first())
            .map(|preset| preset.id.clone())
            .unwrap_or_default();
    }

    pub fn default_preset(&self) -> Option<&WorkspacePreset> {
        self.presets
            .iter()
            .find(|preset| preset.id == self.default_preset_id)
    }

    /// Saves a named standard layout and returns its stable profile ID.
    pub fn add_preset(&mut self, name: &str, layout: WorkspaceLayout) -> Option<String> {
        let name = name.trim();
        if self.presets.len() >= MAX_PRESETS
            || name.is_empty()
            || name.chars().count() > MAX_PRESET_NAME_LENGTH
            || !layout.is_valid()
        {
            return None;
        }

        let base = preset_id_base(name);
        let id = (1..)
            .map(|suffix| {
                let suffix = if suffix == 1 {
                    String::new()
                } else {
                    format!("-{suffix}")
                };
                let base_length = MAX_ID_LENGTH.saturating_sub(suffix.len());
                format!("{}{suffix}", &base[..base.len().min(base_length)])
            })
            .find(|id| self.presets.iter().all(|preset| preset.id != *id))?;

        self.presets.push(WorkspacePreset {
            id: id.clone(),
            name: name.to_owned(),
            layout,
        });
        Some(id)
    }

    /// Updates a preset without changing its stable ID or space assignments.
    pub fn update_preset(&mut self, id: &str, name: &str, layout: WorkspaceLayout) -> bool {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > MAX_PRESET_NAME_LENGTH || !layout.is_valid() {
            return false;
        }
        let Some(preset) = self.presets.iter_mut().find(|preset| preset.id == id) else {
            return false;
        };
        preset.name = name.to_owned();
        preset.layout = layout;
        true
    }

    /// Removes a preset unless it is the last one. If it was the profile
    /// default, the first remaining preset becomes the default.
    pub fn remove_preset(&mut self, id: &str) -> bool {
        if self.presets.len() <= 1 {
            return false;
        }
        let Some(index) = self.presets.iter().position(|preset| preset.id == id) else {
            return false;
        };
        self.presets.remove(index);
        if self.default_preset_id == id {
            self.default_preset_id = self.presets[0].id.clone();
        }
        true
    }

    /// Selects an existing preset as the profile default.
    pub fn set_default_preset(&mut self, id: &str) -> bool {
        if self.default_preset_id == id || !self.presets.iter().any(|preset| preset.id == id) {
            return false;
        }
        self.default_preset_id = id.to_owned();
        true
    }

    /// Checks preset identities, the default reference, and layout structure.
    /// Allowed regions and component-specific extent limits belong to the
    /// shell-owned component catalog.
    pub fn is_valid(&self) -> bool {
        if self.presets.is_empty() || self.presets.len() > MAX_PRESETS {
            return false;
        }

        let mut ids = HashSet::with_capacity(self.presets.len());
        for preset in &self.presets {
            if !is_stable_id(&preset.id)
                || preset.name.trim().is_empty()
                || preset.name.chars().count() > MAX_PRESET_NAME_LENGTH
                || !ids.insert(preset.id.as_str())
                || !preset.layout.is_valid()
            {
                return false;
            }
        }

        ids.contains(self.default_preset_id.as_str())
    }
}

fn preset_id_base(name: &str) -> String {
    let mut id = String::new();
    let mut separator = false;
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            if separator && !id.is_empty() {
                id.push('-');
            }
            id.push(character.to_ascii_lowercase());
            separator = false;
        } else {
            separator = true;
        }
        if id.len() >= MAX_ID_LENGTH {
            break;
        }
    }
    if id.is_empty() {
        "preset".to_owned()
    } else {
        id.trim_end_matches('-').to_owned()
    }
}

fn is_stable_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_ID_LENGTH
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_vertical_and_keeps_both_builtin_presets() {
        let settings = WorkspaceSettings::default();
        assert_eq!(settings.default_preset_id, VERTICAL_PRESET_ID);
        assert!(settings.default_preset().is_some());
        assert!(settings.is_valid());
    }

    #[test]
    fn standard_layouts_support_a_right_sidebar_without_mixing_tab_modes() {
        let layout =
            WorkspaceLayout::for_tab_layout_and_side(TabLayout::Vertical, SidebarSide::Right);
        assert_eq!(layout.tab_layout(), TabLayout::Vertical);
        assert_eq!(layout.sidebar_side(), SidebarSide::Right);
        assert!(layout.is_valid());
        assert!(
            layout
                .component(TAB_STRIP_COMPONENT_ID)
                .is_some_and(|component| !component.visible)
        );
    }

    #[test]
    fn custom_presets_get_stable_unique_ids_and_can_become_the_default() {
        let mut settings = WorkspaceSettings::default();
        let layout =
            WorkspaceLayout::for_tab_layout_and_side(TabLayout::Horizontal, SidebarSide::Right);
        let first = settings
            .add_preset("  Work setup  ", layout.clone())
            .unwrap();
        let second = settings.add_preset("Work setup", layout).unwrap();

        assert_eq!(first, "work-setup");
        assert_eq!(second, "work-setup-2");
        assert!(settings.set_default_preset(&first));
        assert_eq!(settings.default_preset_id, first);
        assert!(settings.is_valid());
    }

    #[test]
    fn presets_can_be_renamed_reconfigured_and_deleted_safely() {
        let mut settings = WorkspaceSettings::default();
        let id = settings
            .add_preset("Research", WorkspaceLayout::horizontal())
            .unwrap();
        assert!(settings.update_preset(
            &id,
            "Research right",
            WorkspaceLayout::for_tab_layout_and_side(TabLayout::Vertical, SidebarSide::Right),
        ));
        assert_eq!(settings.presets.last().unwrap().name, "Research right");
        assert_eq!(
            settings.presets.last().unwrap().layout.tab_layout(),
            TabLayout::Vertical
        );
        assert!(settings.set_default_preset(&id));
        assert!(settings.remove_preset(&id));
        assert_ne!(settings.default_preset_id, id);
        assert!(settings.is_valid());
    }

    #[test]
    fn presets_cannot_delete_the_last_preset() {
        let mut settings = WorkspaceSettings::default();
        let last_preset_id = settings.default_preset_id.clone();
        settings
            .presets
            .retain(|preset| preset.id == last_preset_id);
        assert!(!settings.remove_preset(&last_preset_id));
        assert!(settings.is_valid());
    }

    #[test]
    fn accepts_legacy_builtin_layouts_for_shell_migration() {
        let mut settings = WorkspaceSettings::default();
        settings
            .presets
            .iter_mut()
            .find(|preset| preset.id == HORIZONTAL_PRESET_ID)
            .unwrap()
            .layout = WorkspaceLayout::legacy_horizontal();
        settings
            .presets
            .iter_mut()
            .find(|preset| preset.id == VERTICAL_PRESET_ID)
            .unwrap()
            .layout = WorkspaceLayout::legacy_vertical();

        assert!(settings.is_valid());
    }

    #[test]
    fn maps_legacy_tab_layout_to_a_profile_preset() {
        let settings = WorkspaceSettings::for_tab_layout(TabLayout::Vertical);
        assert_eq!(settings.default_preset_id, VERTICAL_PRESET_ID);
        assert_eq!(
            settings.default_preset().unwrap().layout,
            WorkspaceLayout::vertical()
        );
    }

    #[test]
    fn maps_tab_layout_to_a_remaining_custom_preset_after_builtin_removal() {
        let mut settings = WorkspaceSettings::default();
        settings.remove_preset(VERTICAL_PRESET_ID);
        let right =
            WorkspaceLayout::for_tab_layout_and_side(TabLayout::Vertical, SidebarSide::Right);
        let id = settings.add_preset("Vertical right", right).unwrap();
        settings.remove_preset(VERTICAL_PRESET_ID);
        settings.set_builtin_layout(TabLayout::Vertical);
        assert_eq!(settings.default_preset_id, id);
    }

    #[test]
    fn rejects_duplicate_components_and_gapped_orders() {
        let mut layout = WorkspaceLayout::horizontal();
        layout.components[1].id = TAB_STRIP_COMPONENT_ID.to_owned();
        assert!(!layout.is_valid());

        let mut layout = WorkspaceLayout::horizontal();
        layout.components[1].order = 3;
        assert!(!layout.is_valid());
    }

    #[test]
    fn visibility_is_a_portable_layout_edit() {
        let mut layout = WorkspaceLayout::horizontal();
        assert!(layout.set_component_visible(BROWSER_MENU_BUTTON_COMPONENT_ID, false));
        assert!(!layout.set_component_visible(BROWSER_MENU_BUTTON_COMPONENT_ID, false));
        assert!(layout.set_component_visible(SIDEBAR_TOGGLE_COMPONENT_ID, true));
        assert!(
            !layout
                .component(BROWSER_MENU_BUTTON_COMPONENT_ID)
                .unwrap()
                .visible
        );
    }

    #[test]
    fn adds_components_to_older_layouts() {
        let mut layout = WorkspaceLayout::horizontal();
        assert!(layout.add_component("downloads-panel", DockRegion::Trailing, false));
        assert_eq!(
            layout.component("downloads-panel"),
            Some(&LayoutComponent {
                id: "downloads-panel".to_owned(),
                region: DockRegion::Trailing,
                order: 0,
                visible: false,
                preferred_extent: None,
            })
        );
        assert!(layout.is_valid());
        assert!(!layout.add_component("downloads-panel", DockRegion::Top, false));
    }

    #[test]
    fn moves_components_and_keeps_region_orders_contiguous() {
        let mut layout = WorkspaceLayout::horizontal();
        assert!(layout.move_component(TAB_STRIP_COMPONENT_ID, DockRegion::Bottom, 0));
        assert_eq!(
            layout.component(TAB_STRIP_COMPONENT_ID).unwrap().region,
            DockRegion::Bottom
        );
        assert!(layout.is_valid());
        assert!(!layout.move_component("missing", DockRegion::Top, 0));
    }

    #[test]
    fn rejects_invalid_preset_references_and_extents() {
        let settings = WorkspaceSettings {
            default_preset_id: "missing".to_owned(),
            ..WorkspaceSettings::default()
        };
        assert!(!settings.is_valid());

        let mut layout = WorkspaceLayout::horizontal();
        layout.components[0].preferred_extent = Some(MAX_PREFERRED_EXTENT + 1);
        assert!(!layout.is_valid());
    }
}
