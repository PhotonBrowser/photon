//! The shell's component catalog: each piece of chrome a workspace layout
//! can place, where it may go, and how wide it may be.

use photon_core::{
    ADDRESS_FIELD_COMPONENT_ID, BACK_BUTTON_COMPONENT_ID, BROWSER_MENU_BUTTON_COMPONENT_ID,
    DockRegion, FAVOURITES_PANEL_COMPONENT_ID, FORWARD_BUTTON_COMPONENT_ID,
    RELOAD_BUTTON_COMPONENT_ID, SIDEBAR_DEFAULT_WIDTH, SIDEBAR_FOOTER_COMPONENT_ID,
    SIDEBAR_MAX_WIDTH, SIDEBAR_TOGGLE_COMPONENT_ID, TAB_STRIP_COMPONENT_ID,
    VERTICAL_TABS_COMPONENT_ID,
};

/// A top-level piece of browser chrome known to the layout host.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(in super::super) enum ComponentId {
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
    /// Back, forward, reload and the browser menu, which share the side
    /// dock's top row with its toggle.
    pub(in super::super) const fn is_navigation_control(self) -> bool {
        matches!(
            self,
            Self::BackButton | Self::ForwardButton | Self::ReloadButton | Self::BrowserMenuButton
        )
    }

    pub(in super::super) const fn key(self) -> &'static str {
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

    pub(in super::super) fn from_key(key: &str) -> Option<Self> {
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
pub(in super::super) struct ComponentDefinition {
    pub(in super::super) id: ComponentId,
    pub(in super::super) default_region: DockRegion,
    pub(in super::super) supported_regions: &'static [DockRegion],
    /// Component compatibility groups used to reject conflicting saved layouts.
    pub(in super::super) groups: &'static [&'static str],
    pub(in super::super) preferred_extent: Option<ExtentBounds>,
}

/// A capability group can require or limit visible members, and can keep side
/// members together. Catalog edits automatically participate in these rules.
pub(super) struct ComponentGroupDefinition {
    pub(super) id: &'static str,
    pub(super) minimum_visible: usize,
    pub(super) maximum_visible: usize,
    pub(super) keep_side_members_together: bool,
}

#[derive(Clone, Copy)]
pub(in super::super) struct ExtentBounds {
    pub(in super::super) minimum: u16,
    pub(in super::super) maximum: u16,
}

/// A visible component's resolved placement.
#[derive(Clone, Copy)]
pub(in super::super) struct ComponentPlacement {
    pub(in super::super) id: ComponentId,
    pub(in super::super) region: DockRegion,
}

pub(super) const TOP_BOTTOM: &[DockRegion] = &[DockRegion::Top, DockRegion::Bottom];
pub(super) const SIDES: &[DockRegion] = &[DockRegion::Leading, DockRegion::Trailing];
pub(super) const ALL_DOCKS: &[DockRegion] = &[
    DockRegion::Top,
    DockRegion::Bottom,
    DockRegion::Leading,
    DockRegion::Trailing,
];
pub(super) const TAB_SURFACE: &[&str] = &["tab-surface"];
pub(super) const SIDEBAR_DOCK: &[&str] = &["sidebar-dock"];

pub(super) const COMPONENT_GROUPS: &[ComponentGroupDefinition] = &[
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

pub(super) const COMPONENTS: &[ComponentDefinition] = &[
    ComponentDefinition {
        id: ComponentId::TabStrip,
        default_region: DockRegion::Top,
        supported_regions: TOP_BOTTOM,
        groups: TAB_SURFACE,
        preferred_extent: None,
    },
    ComponentDefinition {
        id: ComponentId::BackButton,
        default_region: DockRegion::Top,
        supported_regions: ALL_DOCKS,
        groups: SIDEBAR_DOCK,
        preferred_extent: None,
    },
    ComponentDefinition {
        id: ComponentId::ForwardButton,
        default_region: DockRegion::Top,
        supported_regions: ALL_DOCKS,
        groups: SIDEBAR_DOCK,
        preferred_extent: None,
    },
    ComponentDefinition {
        id: ComponentId::ReloadButton,
        default_region: DockRegion::Top,
        supported_regions: ALL_DOCKS,
        groups: SIDEBAR_DOCK,
        preferred_extent: None,
    },
    ComponentDefinition {
        id: ComponentId::AddressField,
        default_region: DockRegion::Top,
        supported_regions: ALL_DOCKS,
        groups: &["address-surface", "sidebar-dock"],
        preferred_extent: None,
    },
    ComponentDefinition {
        id: ComponentId::BrowserMenuButton,
        default_region: DockRegion::Top,
        supported_regions: ALL_DOCKS,
        groups: SIDEBAR_DOCK,
        preferred_extent: None,
    },
    ComponentDefinition {
        id: ComponentId::SidebarToggle,
        default_region: DockRegion::Leading,
        supported_regions: SIDES,
        groups: SIDEBAR_DOCK,
        preferred_extent: Some(ExtentBounds {
            minimum: SIDEBAR_DEFAULT_WIDTH as u16,
            maximum: SIDEBAR_MAX_WIDTH as u16,
        }),
    },
    ComponentDefinition {
        id: ComponentId::FavouritesPanel,
        default_region: DockRegion::Leading,
        supported_regions: SIDES,
        groups: SIDEBAR_DOCK,
        preferred_extent: None,
    },
    ComponentDefinition {
        id: ComponentId::VerticalTabs,
        default_region: DockRegion::Leading,
        supported_regions: SIDES,
        groups: &["tab-surface", "sidebar-dock"],
        preferred_extent: None,
    },
    ComponentDefinition {
        id: ComponentId::SidebarFooter,
        default_region: DockRegion::Leading,
        supported_regions: SIDES,
        groups: SIDEBAR_DOCK,
        preferred_extent: None,
    },
];

pub(in super::super) fn component_definition(id: ComponentId) -> &'static ComponentDefinition {
    COMPONENTS
        .iter()
        .find(|definition| definition.id == id)
        .expect("every shell component has a catalog entry")
}
