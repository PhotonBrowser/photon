//! Upgrading layouts saved by older versions to the current catalog.

use photon_core::{
    ADDRESS_FIELD_COMPONENT_ID, ADDRESS_TOOLBAR_COMPONENT_ID, BACK_BUTTON_COMPONENT_ID,
    BROWSER_MENU_BUTTON_COMPONENT_ID, DockRegion, FAVOURITES_PANEL_COMPONENT_ID,
    FORWARD_BUTTON_COMPONENT_ID, LayoutComponent, RELOAD_BUTTON_COMPONENT_ID, SIDEBAR_COMPONENT_ID,
    SIDEBAR_FOOTER_COMPONENT_ID, SIDEBAR_TOGGLE_COMPONENT_ID, TAB_STRIP_COMPONENT_ID,
    VERTICAL_TABS_COMPONENT_ID, WorkspaceLayout as SavedWorkspaceLayout,
};

use super::catalog::*;

/// Converts saved composite components to the current atomic catalog and fills
/// in hidden entries introduced by newer shell versions.
pub(super) fn expand_legacy_components(
    saved: &SavedWorkspaceLayout,
) -> Option<SavedWorkspaceLayout> {
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
                ComponentId::from_key(&component.id)?;
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
