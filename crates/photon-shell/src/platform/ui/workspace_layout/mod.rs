//! Resolves saved workspace data through the shell's component catalog.
//!
//! The core owns portable layout data. This module knows which placements and
//! sizes the current shell can render and supplies safe built-in fallbacks.

use gpui::{App, Window};
use photon_core::{
    DockRegion, Feature, FeatureSettings, SpaceId, TabLayout,
    WorkspaceLayout as SavedWorkspaceLayout, WorkspaceSettings,
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
    /// Resolves the space's named preset or legacy layout, then the profile default.
    pub(super) fn current(space: SpaceId, cx: &App) -> Rc<Self> {
        Settings::layout(space, cx, Self::resolve)
    }

    /// The layout in the space `window` shows.
    pub(super) fn for_window(window: &Window, cx: &App) -> Rc<Self> {
        Self::current(super::window_space::window_space(window, cx).id, cx)
    }

    fn resolve(settings: &photon_core::BrowserSettings, space: SpaceId) -> Self {
        if !settings.features.is_enabled(Feature::Sidebar) {
            return Self::for_tab_layout(TabLayout::Horizontal);
        }
        let active_space = settings
            .spaces
            .get(space)
            .unwrap_or_else(|| settings.spaces.active());
        settings
            .workspace
            .presets
            .iter()
            .find(|preset| active_space.preset_override_id.as_deref() == Some(&preset.id))
            .and_then(|preset| Self::from_saved_with_features(&preset.layout, &settings.features))
            .or_else(|| {
                active_space
                    .layout_override
                    .as_ref()
                    .and_then(|layout| Self::from_saved_with_features(layout, &settings.features))
            })
            .or_else(|| {
                settings.workspace.default_preset().and_then(|preset| {
                    Self::from_saved_with_features(&preset.layout, &settings.features)
                })
            })
            .unwrap_or_else(|| Self::for_tab_layout(settings.tab_layout))
    }

    pub(super) fn from_saved(saved: &SavedWorkspaceLayout) -> Option<Self> {
        Self::from_saved_with_features(saved, &FeatureSettings::default())
    }

    fn from_saved_with_features(
        saved: &SavedWorkspaceLayout,
        features: &FeatureSettings,
    ) -> Option<Self> {
        if !saved.is_valid() {
            return None;
        }
        let mut saved = expand_legacy_components(saved)?;
        for component in &mut saved.components {
            let Some(id) = ComponentId::from_key(&component.id) else {
                continue;
            };
            if component.visible
                && super::window::plugins::component_feature(id)
                    .is_some_and(|feature| !features.is_enabled(feature))
            {
                component.visible = false;
            }
        }

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
    fn every_layout_component_is_declared_by_a_plugin() {
        assert_eq!(
            catalog::COMPONENTS.len(),
            catalog::COMPONENTS
                .iter()
                .filter(|definition| super::super::window::plugins::has_component(definition.id))
                .count()
        );
    }

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
    fn disabling_the_sidebar_uses_the_horizontal_layout_without_changing_preferences() {
        let mut settings = photon_core::BrowserSettings::default();
        settings.features.disable(Feature::Sidebar);
        let space = settings.spaces.active().id;

        let layout = WorkspaceLayout::resolve(&settings, space);
        assert!(!layout.has_sidebar());
        assert!(layout.has_tab_strip());

        settings.features.enable(Feature::Sidebar);
        let restored = WorkspaceLayout::resolve(&settings, space);
        assert!(restored.has_sidebar());
        assert!(restored.has_vertical_tabs());
    }

    #[test]
    fn disabled_component_features_remove_their_owned_layout_components() {
        let saved = SavedWorkspaceLayout::vertical();
        let mut features = FeatureSettings::default();
        features.disable(Feature::Sidebar);

        let resolved = WorkspaceLayout::from_saved_with_features(&saved, &features).unwrap();

        assert!(
            resolved
                .components(DockRegion::Leading)
                .chain(resolved.components(DockRegion::Trailing))
                .all(
                    |placement| super::super::window::plugins::component_feature(placement.id)
                        != Some(Feature::Sidebar)
                )
        );
        assert!(
            resolved
                .saved()
                .component(VERTICAL_TABS_COMPONENT_ID)
                .is_some_and(|component| !component.visible)
        );
        assert!(
            saved
                .component(VERTICAL_TABS_COMPONENT_ID)
                .is_some_and(|component| component.visible)
        );
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
