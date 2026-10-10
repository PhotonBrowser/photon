//! The vertical tab sidebar: navigation, the address field, favourite sites,
//! the current space's tabs, and a footer.
//!
//! These are views of state the browser window owns; the window builds them
//! in `window/sidebar.rs`.
//!
//! - `navigation`: the top row with room for the window controls.
//! - `favourites`: the grid of favourite sites.
//! - `tabs`: the vertical tab list.
//! - `footer`: the space header and the footer.

mod favourites;
mod footer;
mod navigation;
mod tabs;

use gpui::{AnyElement, div, prelude::*, px, rgba};

use super::layout::{h_stack, v_stack};
use super::{metrics, theme::ThemeColors};
pub(super) use favourites::{FavouriteTile, favourites_grid};
pub(super) use footer::{FooterActions, footer, space_header};
pub(super) use navigation::{NavigationActions, NavigationState, navigation_bar};
pub(super) use tabs::{DraggedTab, ICON_ENTRANCE, RevealedIcon, TabIcon, TabItem, tab_list};

/// The sidebar's sections.
pub(super) struct SidebarSections {
    pub(super) navigation: AnyElement,
    pub(super) address: AnyElement,
    /// The favourites grid, when there are favourites.
    pub(super) favourites: Option<AnyElement>,
    pub(super) space: AnyElement,
    pub(super) tabs: AnyElement,
    pub(super) footer: AnyElement,
}

/// Lays the sections out top to bottom: navigation, address and favourites
/// stay put, the space's tabs scroll, and the footer stays at the bottom.
pub(super) fn sidebar(sections: SidebarSections, palette: ThemeColors) -> impl IntoElement {
    let inset = |content: AnyElement| {
        h_stack()
            .w_full()
            .px(px(metrics::SIDEBAR_PADDING))
            .child(content)
    };
    let mut top = v_stack()
        .w_full()
        .flex_shrink_0()
        .gap(px(metrics::SIDEBAR_SECTION_GAP))
        .child(inset(sections.address));
    if let Some(favourites) = sections.favourites {
        top = top.child(inset(favourites));
    }
    v_stack()
        .id("sidebar")
        .size_full()
        .child(sections.navigation)
        .child(top)
        .child(
            div()
                .mx(px(metrics::SIDEBAR_PADDING + metrics::SIDEBAR_TAB_PADDING))
                .my(px(metrics::SIDEBAR_SEPARATOR_MARGIN))
                .h(px(metrics::MENU_SEPARATOR_HEIGHT))
                .bg(rgba(palette.menu_border)),
        )
        .child(
            v_stack()
                .id("sidebar-tabs")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .px(px(metrics::SIDEBAR_PADDING))
                .gap(px(metrics::SIDEBAR_ITEM_GAP))
                .child(sections.space)
                .child(sections.tabs),
        )
        .child(sections.footer)
}
