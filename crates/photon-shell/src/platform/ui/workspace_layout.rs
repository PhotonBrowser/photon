//! The built-in browser workspace layouts and their shell components.
//!
//! The profile still selects the original `TabLayout` value. This module
//! resolves that setting once into a composition definition, keeping layout
//! policy out of individual browser controls and feature views.

use gpui::App;
use photon_core::TabLayout;

use super::settings::Settings;

/// A logical region around the active page where browser chrome can live.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum DockRegion {
    Top,
    Bottom,
    Leading,
    Trailing,
}

/// A top-level piece of browser chrome known to the layout host.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum ComponentId {
    TabStrip,
    AddressToolbar,
    Sidebar,
}

impl ComponentId {
    /// A stable key suitable for a future saved layout representation.
    pub(super) const fn key(self) -> &'static str {
        match self {
            Self::TabStrip => "tab-strip",
            Self::AddressToolbar => "address-toolbar",
            Self::Sidebar => "sidebar",
        }
    }
}

/// A component's shell-owned metadata. Rendering and browser behavior remain
/// with the modules that own each component.
struct ComponentDefinition {
    id: ComponentId,
    name: &'static str,
    default_region: DockRegion,
    supported_regions: &'static [DockRegion],
}

/// A component placed in a region, in reading order.
#[derive(Clone, Copy)]
pub(super) struct ComponentPlacement {
    pub(super) id: ComponentId,
    order: u8,
}

/// How a layout positions its chrome around the active page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Composition {
    /// A stack of chrome components above the page.
    TopStack,
    /// A persistent sidebar beside the page, with an edge reveal while hidden.
    SidebarOverlay,
}

/// The visual treatment used by the address field in this layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OmniboxPlacement {
    Toolbar,
    Sidebar,
}

/// A resolved shell layout. The four regions are present even where a built-in
/// layout leaves one empty, so future layouts can use the same contract.
pub(super) struct WorkspaceLayout {
    composition: Composition,
    omnibox: OmniboxPlacement,
    shows_spaces: bool,
    top: &'static [ComponentPlacement],
    bottom: &'static [ComponentPlacement],
    leading: &'static [ComponentPlacement],
    trailing: &'static [ComponentPlacement],
}

const TOP_ONLY: &[DockRegion] = &[DockRegion::Top];
const LEADING_ONLY: &[DockRegion] = &[DockRegion::Leading];

const COMPONENTS: &[ComponentDefinition] = &[
    ComponentDefinition {
        id: ComponentId::TabStrip,
        name: "Tab strip",
        default_region: DockRegion::Top,
        supported_regions: TOP_ONLY,
    },
    ComponentDefinition {
        id: ComponentId::AddressToolbar,
        name: "Address toolbar",
        default_region: DockRegion::Top,
        supported_regions: TOP_ONLY,
    },
    ComponentDefinition {
        id: ComponentId::Sidebar,
        name: "Sidebar",
        default_region: DockRegion::Leading,
        supported_regions: LEADING_ONLY,
    },
];

const HORIZONTAL_COMPONENTS: &[ComponentPlacement] = &[
    ComponentPlacement {
        id: ComponentId::TabStrip,
        order: 0,
    },
    ComponentPlacement {
        id: ComponentId::AddressToolbar,
        order: 1,
    },
];

const VERTICAL_COMPONENTS: &[ComponentPlacement] = &[ComponentPlacement {
    id: ComponentId::Sidebar,
    order: 0,
}];

const HORIZONTAL: WorkspaceLayout = WorkspaceLayout {
    composition: Composition::TopStack,
    omnibox: OmniboxPlacement::Toolbar,
    shows_spaces: false,
    top: HORIZONTAL_COMPONENTS,
    bottom: &[],
    leading: &[],
    trailing: &[],
};

const VERTICAL: WorkspaceLayout = WorkspaceLayout {
    composition: Composition::SidebarOverlay,
    omnibox: OmniboxPlacement::Sidebar,
    shows_spaces: true,
    top: &[],
    bottom: &[],
    leading: VERTICAL_COMPONENTS,
    trailing: &[],
};

impl WorkspaceLayout {
    /// Resolves the profile's existing layout setting to a built-in workspace.
    pub(super) fn current(cx: &App) -> &'static Self {
        Self::for_tab_layout(Settings::get(cx).tab_layout)
    }

    fn for_tab_layout(tab_layout: TabLayout) -> &'static Self {
        let layout = match tab_layout {
            TabLayout::Horizontal => &HORIZONTAL,
            TabLayout::Vertical => &VERTICAL,
        };
        debug_assert!(layout.is_valid());
        layout
    }

    pub(super) const fn composition(&self) -> Composition {
        self.composition
    }

    pub(super) const fn has_sidebar(&self) -> bool {
        matches!(self.composition, Composition::SidebarOverlay)
    }

    pub(super) const fn shows_spaces(&self) -> bool {
        self.shows_spaces
    }

    pub(super) const fn omnibox_placement(&self) -> OmniboxPlacement {
        self.omnibox
    }

    /// Components in a dock region, already ordered from the window edge in.
    pub(super) const fn components(&self, region: DockRegion) -> &'static [ComponentPlacement] {
        match region {
            DockRegion::Top => self.top,
            DockRegion::Bottom => self.bottom,
            DockRegion::Leading => self.leading,
            DockRegion::Trailing => self.trailing,
        }
    }

    fn is_valid(&self) -> bool {
        let regions = [
            DockRegion::Top,
            DockRegion::Bottom,
            DockRegion::Leading,
            DockRegion::Trailing,
        ];
        let mut seen = 0_u8;

        for region in regions {
            for (index, placement) in self.components(region).iter().enumerate() {
                let Some((catalog_index, definition)) = COMPONENTS
                    .iter()
                    .enumerate()
                    .find(|(_, item)| item.id == placement.id)
                else {
                    return false;
                };
                let component_bit = 1_u8 << catalog_index;
                if definition.name.is_empty()
                    || definition.id.key().is_empty()
                    || !definition.supported_regions.contains(&region)
                    || placement.order as usize != index
                    || (index == 0 && definition.default_region != region)
                    || seen & component_bit != 0
                {
                    return false;
                }
                seen |= component_bit;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_the_existing_layouts_to_their_current_components() {
        assert_eq!(
            HORIZONTAL
                .components(DockRegion::Top)
                .iter()
                .map(|placement| placement.id)
                .collect::<Vec<_>>(),
            [ComponentId::TabStrip, ComponentId::AddressToolbar]
        );
        assert_eq!(HORIZONTAL.composition(), Composition::TopStack);
        assert!(!HORIZONTAL.has_sidebar());
        assert!(!HORIZONTAL.shows_spaces());
        assert_eq!(HORIZONTAL.omnibox_placement(), OmniboxPlacement::Toolbar);

        assert_eq!(
            VERTICAL
                .components(DockRegion::Leading)
                .iter()
                .map(|placement| placement.id)
                .collect::<Vec<_>>(),
            [ComponentId::Sidebar]
        );
        assert_eq!(VERTICAL.composition(), Composition::SidebarOverlay);
        assert!(VERTICAL.has_sidebar());
        assert!(VERTICAL.shows_spaces());
        assert_eq!(VERTICAL.omnibox_placement(), OmniboxPlacement::Sidebar);
    }

    #[test]
    fn built_in_layouts_use_catalog_components_in_supported_regions() {
        assert!(std::ptr::eq(
            WorkspaceLayout::for_tab_layout(TabLayout::Horizontal),
            &HORIZONTAL
        ));
        assert!(std::ptr::eq(
            WorkspaceLayout::for_tab_layout(TabLayout::Vertical),
            &VERTICAL
        ));
        assert!(HORIZONTAL.is_valid());
        assert!(VERTICAL.is_valid());
        assert_eq!(ComponentId::TabStrip.key(), "tab-strip");
        assert_eq!(ComponentId::AddressToolbar.key(), "address-toolbar");
        assert_eq!(ComponentId::Sidebar.key(), "sidebar");
    }
}
