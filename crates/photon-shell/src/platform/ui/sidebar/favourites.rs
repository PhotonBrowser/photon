//! The grid of favourite sites at the top of the sidebar.

use gpui::{AnyElement, App, MouseButton, MouseDownEvent, Role, Window, div, prelude::*, px, rgba};

use super::super::layout::{h_stack, v_stack};
use super::super::{ClickHandler, metrics, theme::ThemeColors};

/// A favourite site's tile.
pub(in super::super) struct FavouriteTile {
    pub(in super::super) label: String,
    /// The site's icon, or a placeholder.
    pub(in super::super) icon: AnyElement,
    /// Whether the active tab shows this site.
    pub(in super::super) active: bool,
    pub(in super::super) on_open: ClickHandler,
    /// Right-click opens the favourite's menu.
    pub(in super::super) on_context_menu: Box<dyn Fn(&MouseDownEvent, &mut Window, &mut App)>,
}

/// Rows of tiles, filling each row so tiles stay the same width.
pub(in super::super) fn favourites_grid(
    tiles: Vec<FavouriteTile>,
    palette: ThemeColors,
) -> impl IntoElement {
    let columns = metrics::SIDEBAR_FAVOURITE_COLUMNS;
    let mut rows = Vec::new();
    let mut tiles = tiles.into_iter().enumerate().peekable();
    while tiles.peek().is_some() {
        let row: Vec<AnyElement> = tiles
            .by_ref()
            .take(columns)
            .map(|(index, tile)| favourite_tile(index, tile, palette))
            .collect();
        let filler = columns - row.len();
        rows.push(
            h_stack()
                .w_full()
                .gap(px(metrics::SIDEBAR_ITEM_GAP * 3.0))
                .children(row)
                .children((0..filler).map(|_| div().flex_1())),
        );
    }
    v_stack()
        .id("sidebar-favourites")
        .role(Role::List)
        .aria_label("Favourites")
        .w_full()
        .gap(px(metrics::SIDEBAR_ITEM_GAP * 3.0))
        .children(rows)
}

fn favourite_tile(index: usize, tile: FavouriteTile, palette: ThemeColors) -> AnyElement {
    let tile_element = h_stack()
        .id(("sidebar-favourite", index))
        .role(Role::ListItem)
        .aria_label(tile.label)
        .aria_selected(tile.active)
        .tab_index(0)
        .focus_visible(|style| style.border_1().border_color(gpui::rgb(palette.chosen)))
        .flex_1()
        .h(px(metrics::SIDEBAR_FAVOURITE_HEIGHT))
        .items_center()
        .justify_center()
        .rounded(px(metrics::SIDEBAR_ITEM_RADIUS))
        .bg(rgba(if tile.active {
            palette.selected_surface
        } else {
            palette.surface
        }))
        .hover(|style| style.bg(rgba(palette.selected_surface)))
        .active(|style| style.bg(rgba(palette.hover_surface)))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Right, tile.on_context_menu)
        .on_click(tile.on_open)
        .child(tile.icon);
    tile_element.into_any_element()
}
