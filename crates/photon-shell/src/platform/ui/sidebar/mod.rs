//! The vertical tab sidebar: navigation, the address field, favourite sites,
//! the tabs, and a footer. The navigation row stays put above the rest,
//! which slides in and out.
//!
//! These are views of state the browser window owns; the window builds them
//! in `window/chrome.rs`.
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
pub(super) use navigation::navigation_bar;

/// One independently placed panel in the side dock.
pub(super) enum SidebarSection {
    Address(AnyElement),
    Favourites(AnyElement),
    Tabs(AnyElement),
    Footer(AnyElement),
}

/// Lays individually selected panels out in their saved order.
pub(super) fn sidebar(sections: Vec<SidebarSection>, palette: ThemeColors) -> impl IntoElement {
    let inset = |content: AnyElement| {
        h_stack()
            .w_full()
            .px(px(metrics::SIDEBAR_PADDING))
            .child(content)
    };
    let mut content = v_stack().id("sidebar").size_full();
    let mut has_fixed_panel = false;
    let mut previous_was_fixed = false;
    for section in sections {
        match section {
            SidebarSection::Address(panel) | SidebarSection::Favourites(panel) => {
                if previous_was_fixed {
                    content =
                        content.child(div().h(px(metrics::SIDEBAR_SECTION_GAP)).flex_shrink_0());
                }
                content = content.child(inset(panel));
                has_fixed_panel = true;
                previous_was_fixed = true;
            }
            SidebarSection::Tabs(tabs) => {
                if has_fixed_panel {
                    content = content.child(
                        div()
                            .mx(px(metrics::SIDEBAR_SEPARATOR_INSET))
                            .my(px(metrics::SIDEBAR_SEPARATOR_MARGIN))
                            .h(px(metrics::MENU_SEPARATOR_HEIGHT))
                            .bg(rgba(palette.menu_border)),
                    );
                }
                content = content.child(
                    v_stack()
                        .id("sidebar-tabs")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .px(px(metrics::SIDEBAR_PADDING))
                        .child(tabs),
                );
                previous_was_fixed = false;
            }
            SidebarSection::Footer(footer) => {
                content = content.child(footer);
                previous_was_fixed = false;
            }
        }
    }
    content
}
