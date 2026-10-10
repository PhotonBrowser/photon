//! The vertical tab sidebar: navigation, the address field, favourite sites,
//! the tabs, and a footer. The navigation row stays put above the rest,
//! which slides in and out.
//!
//! These are views of state the browser window owns; the window builds them
//! in `window/sidebar.rs`.
//!
//! - `navigation`: the top row with room for the window controls.
//! - `favourites`: the grid of favourite sites.
//! - `footer`: settings and the browser menu.
//!
//! The vertical tab list itself lives with the other tab views in `tabs`.

mod favourites;
mod footer;
mod navigation;

use gpui::{AnyElement, div, prelude::*, px, rgba};

use super::layout::{h_stack, v_stack};
use super::{metrics, theme::ThemeColors};
pub(super) use favourites::{FavouriteTile, favourites_grid};
pub(super) use footer::{FooterActions, SpaceDot, footer};
pub(super) use navigation::{NavigationActions, NavigationState, navigation_bar};

/// The sidebar's sections below its navigation row.
pub(super) struct SidebarSections {
    pub(super) address: AnyElement,
    /// The favourites grid, when there are favourites.
    pub(super) favourites: Option<AnyElement>,
    pub(super) tabs: AnyElement,
    pub(super) footer: AnyElement,
}

/// Lays the sections out top to bottom: address and favourites stay put, the
/// tabs scroll, and the footer stays at the bottom.
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
                .child(sections.tabs),
        )
        .child(sections.footer)
}
