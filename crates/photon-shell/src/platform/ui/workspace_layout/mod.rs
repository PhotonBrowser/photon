//! Resolves saved workspace data through the shell's component catalog.
//!
//! The core owns portable layout data. This module knows which placements and
//! sizes the current shell can render and supplies safe built-in fallbacks.

use gpui::App;
use photon_core::{
    DockRegion, TabLayout, WorkspaceLayout as SavedWorkspaceLayout, WorkspaceSettings,
};
use std::rc::Rc;

mod catalog;
mod legacy;

use catalog::*;
pub(super) use catalog::{ComponentId, component_definition};
use legacy::expand_legacy_components;

use super::metrics;
use super::settings::Settings;

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
    /// Resolves the active space's named preset or legacy layout, then the profile default.
    pub(super) fn current(cx: &App) -> Rc<Self> {
        Settings::layout(cx, Self::resolve)
    }

    fn resolve(settings: &photon_core::BrowserSettings) -> Self {
        let active_space = settings.spaces.active();
        settings
            .workspace
            .presets
            .iter()
            .find(|preset| active_space.preset_override_id.as_deref() == Some(&preset.id))
            .and_then(|preset| Self::from_saved(&preset.layout))
            .or_else(|| {
                active_space
                    .layout_override
                    .as_ref()
                    .and_then(Self::from_saved)
            })
            .or_else(|| {
                settings
                    .workspace
                    .default_preset()
                    .and_then(|preset| Self::from_saved(&preset.layout))
            })
            .unwrap_or_else(|| Self::for_tab_layout(settings.tab_layout))
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

    pub(super) fn has_vertical_tabs(&self) -> bool {
        self.components(DockRegion::Leading)
            .chain(self.components(DockRegion::Trailing))
            .any(|component| component.id == ComponentId::VerticalTabs)
    }

    /// Whether tabs show in a strip above or below the page.
    pub(super) fn has_tab_strip(&self) -> bool {
        self.components(DockRegion::Top)
            .chain(self.components(DockRegion::Bottom))
            .any(|component| component.id == ComponentId::TabStrip)
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

#[cfg(test)]
mod tests {
    use super::*;
    use photon_core::{
        ADDRESS_FIELD_COMPONENT_ID, ADDRESS_TOOLBAR_COMPONENT_ID, LayoutComponent,
        SIDEBAR_COMPONENT_ID, SIDEBAR_TOGGLE_COMPONENT_ID, TAB_STRIP_COMPONENT_ID,
        VERTICAL_TABS_COMPONENT_ID,
    };

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
