//! Resolves saved workspace data through the shell's component catalog.
//!
//! The core owns portable layout data. This module knows which placements and
//! sizes the current shell can render and supplies safe built-in fallbacks.

use gpui::App;
use photon_core::{
    ADDRESS_FIELD_COMPONENT_ID, ADDRESS_TOOLBAR_COMPONENT_ID, BACK_BUTTON_COMPONENT_ID,
    BROWSER_MENU_BUTTON_COMPONENT_ID, DockRegion, FAVOURITES_PANEL_COMPONENT_ID,
    FORWARD_BUTTON_COMPONENT_ID, LayoutComponent, RELOAD_BUTTON_COMPONENT_ID, SIDEBAR_COMPONENT_ID,
    SIDEBAR_DEFAULT_WIDTH, SIDEBAR_FOOTER_COMPONENT_ID, SIDEBAR_MAX_WIDTH,
    SIDEBAR_TOGGLE_COMPONENT_ID, TAB_STRIP_COMPONENT_ID, TabLayout, VERTICAL_TABS_COMPONENT_ID,
    WorkspaceLayout as SavedWorkspaceLayout, WorkspaceSettings,
};

use super::metrics;
use super::settings::Settings;

/// A top-level piece of browser chrome known to the layout host.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum ComponentId {
    TabStrip,
    BackButton,
    ForwardButton,
    ReloadButton,
    AddressField,
    BrowserMenuButton,
    SidebarToggle,
    FavouritesPanel,
    VerticalTabs,
    SidebarFooter,
}

impl ComponentId {
    pub(super) const fn key(self) -> &'static str {
        match self {
            Self::TabStrip => TAB_STRIP_COMPONENT_ID,
            Self::BackButton => BACK_BUTTON_COMPONENT_ID,
            Self::ForwardButton => FORWARD_BUTTON_COMPONENT_ID,
            Self::ReloadButton => RELOAD_BUTTON_COMPONENT_ID,
            Self::AddressField => ADDRESS_FIELD_COMPONENT_ID,
            Self::BrowserMenuButton => BROWSER_MENU_BUTTON_COMPONENT_ID,
            Self::SidebarToggle => SIDEBAR_TOGGLE_COMPONENT_ID,
            Self::FavouritesPanel => FAVOURITES_PANEL_COMPONENT_ID,
            Self::VerticalTabs => VERTICAL_TABS_COMPONENT_ID,
            Self::SidebarFooter => SIDEBAR_FOOTER_COMPONENT_ID,
        }
    }

    pub(super) fn from_key(key: &str) -> Option<Self> {
        match key {
            TAB_STRIP_COMPONENT_ID => Some(Self::TabStrip),
            BACK_BUTTON_COMPONENT_ID => Some(Self::BackButton),
            FORWARD_BUTTON_COMPONENT_ID => Some(Self::ForwardButton),
            RELOAD_BUTTON_COMPONENT_ID => Some(Self::ReloadButton),
            ADDRESS_FIELD_COMPONENT_ID => Some(Self::AddressField),
            BROWSER_MENU_BUTTON_COMPONENT_ID => Some(Self::BrowserMenuButton),
            SIDEBAR_TOGGLE_COMPONENT_ID => Some(Self::SidebarToggle),
            FAVOURITES_PANEL_COMPONENT_ID => Some(Self::FavouritesPanel),
            VERTICAL_TABS_COMPONENT_ID => Some(Self::VerticalTabs),
            SIDEBAR_FOOTER_COMPONENT_ID => Some(Self::SidebarFooter),
            _ => None,
        }
    }
}

/// A shell component's placement and size limits.
pub(super) struct ComponentDefinition {
    pub(super) id: ComponentId,
    pub(super) name: &'static str,
    pub(super) description: &'static str,
    pub(super) default_region: DockRegion,
    pub(super) supported_regions: &'static [DockRegion],
    /// Catalog-level constraints shared by validation, drag targets and edits.
    pub(super) groups: &'static [&'static str],
    pub(super) preferred_extent: Option<ExtentBounds>,
}

/// A capability group can require or limit visible members, and can keep side
/// members together. Catalog edits automatically participate in these rules.
struct ComponentGroupDefinition {
    id: &'static str,
    minimum_visible: usize,
    maximum_visible: usize,
    keep_side_members_together: bool,
}

#[derive(Clone, Copy)]
pub(super) struct ExtentBounds {
    pub(super) minimum: u16,
    pub(super) maximum: u16,
    pub(super) default: u16,
}

/// A visible component's resolved placement.
#[derive(Clone, Copy)]
pub(super) struct ComponentPlacement {
    pub(super) id: ComponentId,
    pub(super) region: DockRegion,
}

const TOP_BOTTOM: &[DockRegion] = &[DockRegion::Top, DockRegion::Bottom];
const SIDES: &[DockRegion] = &[DockRegion::Leading, DockRegion::Trailing];
const ALL_DOCKS: &[DockRegion] = &[
    DockRegion::Top,
    DockRegion::Bottom,
    DockRegion::Leading,
    DockRegion::Trailing,
];
const TAB_SURFACE: &[&str] = &["tab-surface"];
const SIDEBAR_DOCK: &[&str] = &["sidebar-dock"];

const COMPONENT_GROUPS: &[ComponentGroupDefinition] = &[
    ComponentGroupDefinition {
        id: "tab-surface",
        minimum_visible: 0,
        maximum_visible: 1,
        keep_side_members_together: false,
    },
    ComponentGroupDefinition {
        id: "address-surface",
        minimum_visible: 0,
        maximum_visible: 1,
        keep_side_members_together: false,
    },
    ComponentGroupDefinition {
        id: "sidebar-dock",
        minimum_visible: 0,
        maximum_visible: usize::MAX,
        keep_side_members_together: true,
    },
];

const COMPONENTS: &[ComponentDefinition] = &[
    ComponentDefinition {
        id: ComponentId::TabStrip,
        name: "Tab strip",
        description: "Horizontal tabs across the top or bottom edge.",
        default_region: DockRegion::Top,
        supported_regions: TOP_BOTTOM,
        groups: TAB_SURFACE,
        preferred_extent: None,
    },
    ComponentDefinition {
        id: ComponentId::BackButton,
        name: "Back button",
        description: "Go back in the active tab's history.",
        default_region: DockRegion::Top,
        supported_regions: ALL_DOCKS,
        groups: SIDEBAR_DOCK,
        preferred_extent: None,
    },
    ComponentDefinition {
        id: ComponentId::ForwardButton,
        name: "Forward button",
        description: "Go forward in the active tab's history.",
        default_region: DockRegion::Top,
        supported_regions: ALL_DOCKS,
        groups: SIDEBAR_DOCK,
        preferred_extent: None,
    },
    ComponentDefinition {
        id: ComponentId::ReloadButton,
        name: "Reload button",
        description: "Reload or stop the active page.",
        default_region: DockRegion::Top,
        supported_regions: ALL_DOCKS,
        groups: SIDEBAR_DOCK,
        preferred_extent: None,
    },
    ComponentDefinition {
        id: ComponentId::AddressField,
        name: "Address field",
        description: "Enter a website address or search.",
        default_region: DockRegion::Top,
        supported_regions: ALL_DOCKS,
        groups: &["address-surface", "sidebar-dock"],
        preferred_extent: None,
    },
    ComponentDefinition {
        id: ComponentId::BrowserMenuButton,
        name: "Browser menu",
        description: "Open browser commands and settings.",
        default_region: DockRegion::Top,
        supported_regions: ALL_DOCKS,
        groups: SIDEBAR_DOCK,
        preferred_extent: None,
    },
    ComponentDefinition {
        id: ComponentId::SidebarToggle,
        name: "Sidebar toggle",
        description: "Show or hide the side dock.",
        default_region: DockRegion::Leading,
        supported_regions: SIDES,
        groups: SIDEBAR_DOCK,
        preferred_extent: Some(ExtentBounds {
            minimum: SIDEBAR_DEFAULT_WIDTH as u16,
            maximum: SIDEBAR_MAX_WIDTH as u16,
            default: SIDEBAR_DEFAULT_WIDTH as u16,
        }),
    },
    ComponentDefinition {
        id: ComponentId::FavouritesPanel,
        name: "Favourites panel",
        description: "Show favourite websites in a compact grid.",
        default_region: DockRegion::Leading,
        supported_regions: SIDES,
        groups: SIDEBAR_DOCK,
        preferred_extent: None,
    },
    ComponentDefinition {
        id: ComponentId::VerticalTabs,
        name: "Vertical tabs",
        description: "Show the space's tabs in a vertical list.",
        default_region: DockRegion::Leading,
        supported_regions: SIDES,
        groups: &["tab-surface", "sidebar-dock"],
        preferred_extent: None,
    },
    ComponentDefinition {
        id: ComponentId::SidebarFooter,
        name: "Sidebar footer",
        description: "Show settings, spaces and the browser menu.",
        default_region: DockRegion::Leading,
        supported_regions: SIDES,
        groups: SIDEBAR_DOCK,
        preferred_extent: None,
    },
];

pub(super) fn component_catalog() -> &'static [ComponentDefinition] {
    COMPONENTS
}

pub(super) fn component_definition(id: ComponentId) -> &'static ComponentDefinition {
    COMPONENTS
        .iter()
        .find(|definition| definition.id == id)
        .expect("every shell component has a catalog entry")
}

/// A resolved workspace layout supported by the current shell.
#[derive(Clone)]
pub(super) struct WorkspaceLayout {
    saved: SavedWorkspaceLayout,
}

/// The visual treatment used by the address field in this layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OmniboxPlacement {
    Toolbar,
    Sidebar,
}

impl WorkspaceLayout {
    /// Resolves the active space's copied layout, then the profile default.
    pub(super) fn current(cx: &App) -> Self {
        let settings = Settings::get(cx);
        settings
            .spaces
            .active()
            .layout_override
            .as_ref()
            .and_then(Self::from_saved)
            .or_else(|| {
                settings
                    .workspace
                    .default_preset()
                    .and_then(|preset| Self::from_saved(&preset.layout))
            })
            .unwrap_or_else(|| {
                let fallback = WorkspaceSettings::for_tab_layout(settings.tab_layout);
                Self::from_saved(&fallback.default_preset().expect("built-in preset").layout)
                    .expect("built-in layouts are supported by the shell")
            })
    }

    pub(super) fn from_saved(saved: &SavedWorkspaceLayout) -> Option<Self> {
        if !saved.is_valid() {
            return None;
        }
        let saved = expand_legacy_components(saved)?;

        let visible = saved
            .components
            .iter()
            .filter(|component| component.visible);
        let mut group_counts = std::collections::HashMap::new();
        for component in visible {
            let id = ComponentId::from_key(&component.id)?;
            let definition = component_definition(id);
            if !definition.supported_regions.contains(&component.region)
                || (component.preferred_extent.is_some() && definition.preferred_extent.is_none())
                || component.preferred_extent.is_some_and(|extent| {
                    definition
                        .preferred_extent
                        .is_some_and(|bounds| extent < bounds.minimum || extent > bounds.maximum)
                })
            {
                return None;
            }
            for group in definition.groups {
                *group_counts.entry(*group).or_insert(0usize) += 1;
            }
        }

        for group in COMPONENT_GROUPS
            .iter()
            .filter(|group| group.keep_side_members_together)
        {
            let side_regions = saved
                .components
                .iter()
                .filter(|component| component.visible)
                .filter(|component| {
                    ComponentId::from_key(&component.id)
                        .is_some_and(|id| component_definition(id).groups.contains(&group.id))
                })
                .filter(|component| {
                    matches!(component.region, DockRegion::Leading | DockRegion::Trailing)
                })
                .map(|component| component.region)
                .collect::<std::collections::HashSet<_>>();
            if side_regions.len() > 1 {
                return None;
            }
        }

        if COMPONENT_GROUPS.iter().any(|group| {
            let count = group_counts.get(group.id).copied().unwrap_or_default();
            count < group.minimum_visible || count > group.maximum_visible
        }) {
            return None;
        }

        Some(Self { saved })
    }

    pub(super) fn for_tab_layout(layout: TabLayout) -> Self {
        let settings = WorkspaceSettings::for_tab_layout(layout);
        let saved = settings
            .default_preset()
            .expect("built-in preset")
            .layout
            .clone();
        Self::from_saved(&saved).expect("built-in layouts are supported by the shell")
    }

    pub(super) fn components(
        &self,
        region: DockRegion,
    ) -> impl Iterator<Item = ComponentPlacement> + '_ {
        self.saved
            .components
            .iter()
            .filter(move |component| component.visible && component.region == region)
            .filter_map(|component| {
                ComponentId::from_key(&component.id).map(|id| ComponentPlacement {
                    id,
                    region: component.region,
                })
            })
    }

    pub(super) fn has_sidebar(&self) -> bool {
        self.sidebar_region().is_some()
    }

    pub(super) fn sidebar_region(&self) -> Option<DockRegion> {
        self.components(DockRegion::Leading)
            .chain(self.components(DockRegion::Trailing))
            .next()
            .map(|component| component.region)
    }

    pub(super) fn sidebar_extent(&self) -> Option<u32> {
        let region = self.sidebar_region()?;
        self.saved
            .components
            .iter()
            .find(|component| {
                component.id == ComponentId::SidebarToggle.key() && component.region == region
            })
            .and_then(|component| component.preferred_extent)
            .map(u32::from)
    }

    pub(super) fn shows_spaces(&self) -> bool {
        self.components(DockRegion::Leading)
            .chain(self.components(DockRegion::Trailing))
            .any(|component| component.id == ComponentId::SidebarFooter)
    }

    pub(super) fn omnibox_placement(&self) -> OmniboxPlacement {
        if self
            .components(DockRegion::Leading)
            .chain(self.components(DockRegion::Trailing))
            .any(|component| component.id == ComponentId::AddressField)
        {
            OmniboxPlacement::Sidebar
        } else {
            OmniboxPlacement::Toolbar
        }
    }

    /// Height reserved above the page, including a native titlebar when no
    /// tab strip occupies it.
    pub(super) fn top_extent(&self) -> f32 {
        let components: Vec<_> = self.components(DockRegion::Top).collect();
        let has_tab_strip = components
            .iter()
            .any(|component| component.id == ComponentId::TabStrip);
        let has_toolbar = components
            .iter()
            .any(|component| component.id != ComponentId::TabStrip);
        let needs_native_titlebar = has_tab_strip || !self.has_sidebar() || has_toolbar;
        (if has_toolbar {
            metrics::TOOLBAR_HEIGHT
        } else {
            0.0
        }) + if needs_native_titlebar {
            metrics::TITLEBAR_HEIGHT
        } else {
            0.0
        }
    }

    pub(super) fn bottom_extent(&self) -> f32 {
        let components: Vec<_> = self.components(DockRegion::Bottom).collect();
        let has_tabs = components
            .iter()
            .any(|component| component.id == ComponentId::TabStrip);
        let has_toolbar = components
            .iter()
            .any(|component| component.id != ComponentId::TabStrip);
        (if has_tabs {
            metrics::TITLEBAR_HEIGHT
        } else {
            0.0
        }) + if has_toolbar {
            metrics::TOOLBAR_HEIGHT
        } else {
            0.0
        }
    }

    pub(super) fn saved(&self) -> &SavedWorkspaceLayout {
        &self.saved
    }
}

/// Converts saved composite components to the current atomic catalog and fills
/// in hidden entries introduced by newer shell versions.
fn expand_legacy_components(saved: &SavedWorkspaceLayout) -> Option<SavedWorkspaceLayout> {
    let legacy_toolbar = saved.component(ADDRESS_TOOLBAR_COMPONENT_ID);
    let legacy_sidebar = saved.component(SIDEBAR_COMPONENT_ID);
    let legacy_sidebar_visible = legacy_sidebar.is_some_and(|component| component.visible);
    let legacy_toolbar_visible = legacy_toolbar.is_some_and(|component| component.visible);
    let has_legacy_components = legacy_toolbar.is_some() || legacy_sidebar.is_some();
    if legacy_sidebar_visible && legacy_toolbar_visible {
        return None;
    }

    if has_legacy_components
        && saved.components.iter().any(|component| {
            ComponentId::from_key(&component.id).is_some() && component.id != TAB_STRIP_COMPONENT_ID
        })
    {
        return None;
    }

    let mut expanded = SavedWorkspaceLayout::default();
    for component in &saved.components {
        match component.id.as_str() {
            ADDRESS_TOOLBAR_COMPONENT_ID => {
                if component.visible && !legacy_sidebar_visible {
                    for id in [
                        BACK_BUTTON_COMPONENT_ID,
                        FORWARD_BUTTON_COMPONENT_ID,
                        RELOAD_BUTTON_COMPONENT_ID,
                        ADDRESS_FIELD_COMPONENT_ID,
                        BROWSER_MENU_BUTTON_COMPONENT_ID,
                    ] {
                        expanded.components.push(LayoutComponent {
                            id: id.to_owned(),
                            region: component.region,
                            order: 0,
                            visible: true,
                            preferred_extent: None,
                        });
                    }
                }
            }
            SIDEBAR_COMPONENT_ID => {
                let side_ids: &[&str] = if component.visible {
                    &[
                        SIDEBAR_TOGGLE_COMPONENT_ID,
                        BACK_BUTTON_COMPONENT_ID,
                        FORWARD_BUTTON_COMPONENT_ID,
                        RELOAD_BUTTON_COMPONENT_ID,
                        ADDRESS_FIELD_COMPONENT_ID,
                        FAVOURITES_PANEL_COMPONENT_ID,
                        VERTICAL_TABS_COMPONENT_ID,
                        SIDEBAR_FOOTER_COMPONENT_ID,
                    ]
                } else {
                    &[
                        SIDEBAR_TOGGLE_COMPONENT_ID,
                        FAVOURITES_PANEL_COMPONENT_ID,
                        VERTICAL_TABS_COMPONENT_ID,
                        SIDEBAR_FOOTER_COMPONENT_ID,
                    ]
                };
                for id in side_ids {
                    expanded.components.push(LayoutComponent {
                        id: (*id).to_owned(),
                        region: component.region,
                        order: 0,
                        visible: component.visible,
                        preferred_extent: if *id == SIDEBAR_TOGGLE_COMPONENT_ID {
                            component.preferred_extent
                        } else {
                            None
                        },
                    });
                }
            }
            _ => {
                if ComponentId::from_key(&component.id).is_none() {
                    return None;
                }
                expanded.components.push(component.clone());
            }
        }
    }

    for definition in COMPONENTS {
        if expanded.component(definition.id.key()).is_some() {
            continue;
        }
        let visible = legacy_sidebar_visible
            && matches!(
                definition.id,
                ComponentId::BackButton
                    | ComponentId::ForwardButton
                    | ComponentId::ReloadButton
                    | ComponentId::AddressField
                    | ComponentId::FavouritesPanel
                    | ComponentId::VerticalTabs
                    | ComponentId::SidebarFooter
                    | ComponentId::SidebarToggle
            );
        expanded.components.push(LayoutComponent {
            id: definition.id.key().to_owned(),
            region: if visible {
                legacy_sidebar.map_or(definition.default_region, |sidebar| sidebar.region)
            } else {
                definition.default_region
            },
            order: 0,
            visible,
            preferred_extent: None,
        });
    }

    for region in [
        DockRegion::Top,
        DockRegion::Bottom,
        DockRegion::Leading,
        DockRegion::Trailing,
    ] {
        let mut indices = expanded
            .components
            .iter()
            .enumerate()
            .filter(|(_, component)| component.region == region)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        indices.sort_by_key(|&index| expanded.components[index].order);
        for (order, index) in indices.into_iter().enumerate() {
            expanded.components[index].order = order as u16;
        }
    }

    expanded.is_valid().then_some(expanded)
}

pub(super) fn component_group_bounds(id: &str) -> Option<(usize, usize)> {
    COMPONENT_GROUPS
        .iter()
        .find(|group| group.id == id)
        .map(|group| (group.minimum_visible, group.maximum_visible))
}

pub(super) fn component_group_keeps_side_members_together(id: &str) -> bool {
    COMPONENT_GROUPS
        .iter()
        .find(|group| group.id == id)
        .is_some_and(|group| group.keep_side_members_together)
}

pub(super) fn component_groups() -> impl Iterator<Item = (&'static str, usize, usize)> {
    COMPONENT_GROUPS
        .iter()
        .map(|group| (group.id, group.minimum_visible, group.maximum_visible))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_the_builtin_layouts_to_their_current_components() {
        let horizontal = WorkspaceLayout::for_tab_layout(TabLayout::Horizontal);
        assert_eq!(
            horizontal
                .components(DockRegion::Top)
                .map(|component| component.id)
                .collect::<Vec<_>>(),
            [
                ComponentId::TabStrip,
                ComponentId::BackButton,
                ComponentId::ForwardButton,
                ComponentId::ReloadButton,
                ComponentId::AddressField,
                ComponentId::BrowserMenuButton,
            ]
        );
        assert!(!horizontal.has_sidebar());
        assert!(!horizontal.shows_spaces());
        assert_eq!(horizontal.omnibox_placement(), OmniboxPlacement::Toolbar);
        assert_eq!(
            horizontal.top_extent(),
            metrics::TITLEBAR_HEIGHT + metrics::TOOLBAR_HEIGHT
        );

        let vertical = WorkspaceLayout::for_tab_layout(TabLayout::Vertical);
        assert_eq!(
            vertical
                .components(DockRegion::Leading)
                .map(|component| component.id)
                .collect::<Vec<_>>(),
            [
                ComponentId::SidebarToggle,
                ComponentId::BackButton,
                ComponentId::ForwardButton,
                ComponentId::ReloadButton,
                ComponentId::AddressField,
                ComponentId::FavouritesPanel,
                ComponentId::VerticalTabs,
                ComponentId::SidebarFooter,
            ]
        );
        assert!(vertical.has_sidebar());
        assert!(vertical.shows_spaces());
        assert_eq!(vertical.omnibox_placement(), OmniboxPlacement::Sidebar);
        assert_eq!(vertical.top_extent(), 0.0);
    }

    #[test]
    fn rejects_unsupported_placements_and_conflicting_tab_surfaces() {
        let mut layout = SavedWorkspaceLayout::horizontal();
        layout.components[0].region = DockRegion::Trailing;
        assert!(WorkspaceLayout::from_saved(&layout).is_none());

        let mut layout = SavedWorkspaceLayout::horizontal();
        assert!(layout.set_component_visible(VERTICAL_TABS_COMPONENT_ID, true));
        assert!(WorkspaceLayout::from_saved(&layout).is_none());
    }

    #[test]
    fn supports_toolbar_on_bottom_and_sidebar_on_trailing_edge() {
        let mut layout = SavedWorkspaceLayout::horizontal();
        assert!(layout.set_component_visible(TAB_STRIP_COMPONENT_ID, false));
        assert!(layout.move_component(ADDRESS_FIELD_COMPONENT_ID, DockRegion::Bottom, 0));
        assert!(layout.set_component_visible(SIDEBAR_TOGGLE_COMPONENT_ID, true));
        assert!(layout.move_component(SIDEBAR_TOGGLE_COMPONENT_ID, DockRegion::Trailing, 0));
        assert!(WorkspaceLayout::from_saved(&layout).is_some());
    }

    #[test]
    fn rejects_visible_side_components_split_across_edges() {
        let mut layout = SavedWorkspaceLayout::vertical();
        assert!(layout.move_component(VERTICAL_TABS_COMPONENT_ID, DockRegion::Trailing, 0));
        assert!(WorkspaceLayout::from_saved(&layout).is_none());
    }

    #[test]
    fn expands_legacy_composite_layouts_without_changing_their_default_ui() {
        let legacy_horizontal = SavedWorkspaceLayout {
            components: vec![
                LayoutComponent {
                    id: TAB_STRIP_COMPONENT_ID.to_owned(),
                    region: DockRegion::Top,
                    order: 0,
                    visible: true,
                    preferred_extent: None,
                },
                LayoutComponent {
                    id: ADDRESS_TOOLBAR_COMPONENT_ID.to_owned(),
                    region: DockRegion::Top,
                    order: 1,
                    visible: true,
                    preferred_extent: None,
                },
                LayoutComponent {
                    id: SIDEBAR_COMPONENT_ID.to_owned(),
                    region: DockRegion::Leading,
                    order: 0,
                    visible: false,
                    preferred_extent: None,
                },
            ],
        };
        let horizontal = WorkspaceLayout::from_saved(&legacy_horizontal).unwrap();
        assert_eq!(
            horizontal
                .components(DockRegion::Top)
                .map(|component| component.id)
                .collect::<Vec<_>>(),
            [
                ComponentId::TabStrip,
                ComponentId::BackButton,
                ComponentId::ForwardButton,
                ComponentId::ReloadButton,
                ComponentId::AddressField,
                ComponentId::BrowserMenuButton,
            ]
        );
        assert!(!horizontal.has_sidebar());
        assert_eq!(
            horizontal.top_extent(),
            metrics::TITLEBAR_HEIGHT + metrics::TOOLBAR_HEIGHT
        );

        let legacy_vertical = SavedWorkspaceLayout {
            components: vec![
                LayoutComponent {
                    id: TAB_STRIP_COMPONENT_ID.to_owned(),
                    region: DockRegion::Top,
                    order: 0,
                    visible: false,
                    preferred_extent: None,
                },
                LayoutComponent {
                    id: ADDRESS_TOOLBAR_COMPONENT_ID.to_owned(),
                    region: DockRegion::Top,
                    order: 1,
                    visible: false,
                    preferred_extent: None,
                },
                LayoutComponent {
                    id: SIDEBAR_COMPONENT_ID.to_owned(),
                    region: DockRegion::Leading,
                    order: 0,
                    visible: true,
                    preferred_extent: None,
                },
            ],
        };
        let vertical = WorkspaceLayout::from_saved(&legacy_vertical).unwrap();
        assert_eq!(
            vertical
                .components(DockRegion::Leading)
                .map(|component| component.id)
                .collect::<Vec<_>>(),
            [
                ComponentId::SidebarToggle,
                ComponentId::BackButton,
                ComponentId::ForwardButton,
                ComponentId::ReloadButton,
                ComponentId::AddressField,
                ComponentId::FavouritesPanel,
                ComponentId::VerticalTabs,
                ComponentId::SidebarFooter,
            ]
        );
        assert!(vertical.shows_spaces());
        assert_eq!(vertical.omnibox_placement(), OmniboxPlacement::Sidebar);
        assert_eq!(vertical.top_extent(), 0.0);
    }
}
