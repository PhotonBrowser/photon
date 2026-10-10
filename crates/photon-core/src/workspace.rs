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

    fn legacy_horizontal() -> Self {
        Self {
            components: vec![
                LayoutComponent::new(TAB_STRIP_COMPONENT_ID, DockRegion::Top, 0, true),
                LayoutComponent::new(ADDRESS_TOOLBAR_COMPONENT_ID, DockRegion::Top, 1, true),
                LayoutComponent::new(SIDEBAR_COMPONENT_ID, DockRegion::Leading, 0, false),
            ],
        }
    }

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
            default_preset_id: HORIZONTAL_PRESET_ID.to_owned(),
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
        self.default_preset_id = match tab_layout {
            TabLayout::Horizontal => HORIZONTAL_PRESET_ID,
            TabLayout::Vertical => VERTICAL_PRESET_ID,
        }
        .to_owned();
    }

    pub fn default_preset(&self) -> Option<&WorkspacePreset> {
        self.presets
            .iter()
            .find(|preset| preset.id == self.default_preset_id)
    }

    /// Checks preset identities, the default reference, and layout structure.
    /// Allowed regions and component-specific extent limits belong to the
    /// shell-owned component catalog.
    pub fn is_valid(&self) -> bool {
        if self.presets.len() < 2 || self.presets.len() > MAX_PRESETS {
            return false;
        }

        let mut ids = HashSet::with_capacity(self.presets.len());
        let mut has_horizontal = false;
        let mut has_vertical = false;
        for preset in &self.presets {
            if !is_stable_id(&preset.id)
                || preset.name.trim().is_empty()
                || preset.name.chars().count() > MAX_PRESET_NAME_LENGTH
                || !ids.insert(preset.id.as_str())
                || !preset.layout.is_valid()
            {
                return false;
            }

            match preset.id.as_str() {
                HORIZONTAL_PRESET_ID => {
                    has_horizontal = preset.layout == WorkspaceLayout::horizontal()
                        || preset.layout == WorkspaceLayout::legacy_horizontal();
                    if !has_horizontal {
                        return false;
                    }
                }
                VERTICAL_PRESET_ID => {
                    has_vertical = preset.layout == WorkspaceLayout::vertical()
                        || preset.layout == WorkspaceLayout::legacy_vertical();
                    if !has_vertical {
                        return false;
                    }
                }
                _ => {}
            }
        }

        has_horizontal && has_vertical && ids.contains(self.default_preset_id.as_str())
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
    fn defaults_to_horizontal_and_keeps_both_builtin_presets() {
        let settings = WorkspaceSettings::default();
        assert_eq!(settings.default_preset_id, HORIZONTAL_PRESET_ID);
        assert!(settings.default_preset().is_some());
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
        let mut settings = WorkspaceSettings::default();
        settings.default_preset_id = "missing".to_owned();
        assert!(!settings.is_valid());

        let mut layout = WorkspaceLayout::horizontal();
        layout.components[0].preferred_extent = Some(MAX_PREFERRED_EXTENT + 1);
        assert!(!layout.is_valid());
    }
}
