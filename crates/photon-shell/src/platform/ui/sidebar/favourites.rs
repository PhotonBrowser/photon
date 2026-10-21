//! The grid of favourite sites at the top of the sidebar.

use gpui::{AnyElement, FontWeight, MouseButton, Role, div, prelude::*, px, rgb, rgba};

use super::super::button::icon_button;
use super::super::controls::FocusRing;
use super::super::controls::{OwnsClicks, Pressable};
use super::super::icons::{close_icon, favourite_star_icon};
use super::super::layout::{h_stack, v_stack};
use super::super::tabs::{DraggedTab, TabDrop, TabPrepaint};
use super::super::tooltip::label_tooltip;
use super::super::{ClickHandler, MouseDownHandler, metrics, theme::ThemeColors};
use super::collapsed_tile;

/// A favourite site's tile.
pub(in super::super) struct FavouriteTile {
    pub(in super::super) label: String,
    /// The site's icon, or a placeholder.
    pub(in super::super) icon: AnyElement,
    /// Whether the active tab shows this site.
    pub(in super::super) active: bool,
    pub(in super::super) on_open: ClickHandler,
    /// Right-click opens the favourite's menu.
    pub(in super::super) on_context_menu: MouseDownHandler,
}

/// Rows of tiles, filling each row so tiles stay the same width. Collapsed
/// to icons, a column of square tiles.
pub(in super::super) fn favourites_grid(
    tiles: Vec<FavouriteTile>,
    collapsed: bool,
    palette: ThemeColors,
    on_drop: TabDrop,
    on_prepaint: TabPrepaint,
    suppress_tooltips: bool,
) -> impl IntoElement {
    let columns = if collapsed {
        1
    } else {
        metrics::SIDEBAR_FAVOURITE_COLUMNS
    };
    let gap = metrics::SidebarSpacing::new(collapsed).favourite_gap;
    let mut rows = Vec::new();
    let mut tiles = tiles.into_iter().enumerate().peekable();
    while tiles.peek().is_some() {
        let row: Vec<AnyElement> = tiles
            .by_ref()
            .take(columns)
            .map(|(index, tile)| favourite_tile(index, tile, collapsed, suppress_tooltips, palette))
            .collect();
        let filler = columns - row.len();
        rows.push(
            h_stack()
                .w_full()
                .gap(px(gap))
                .children(row)
                .children((0..filler).map(|_| div().flex_1())),
        );
    }
    v_stack()
        .id("sidebar-favourites")
        .role(Role::List)
        .aria_label("Favourites")
        .w_full()
        .gap(px(gap))
        .on_drag_over::<DraggedTab>(move |style, _, _, _| {
            style
                .border_1()
                .border_color(rgba(palette.soft_accent_border))
        })
        .on_drop(move |dragged: &DraggedTab, window, cx| on_drop(dragged, window, cx))
        .on_prepaint(move |prepaint, window, cx| on_prepaint(prepaint.bounds, window, cx))
        .children(rows)
}

/// Empty favourites area, which teaches new users to menu-favourite a tab.
pub(in super::super) fn favourites_empty_state(
    palette: ThemeColors,
    on_dismiss: ClickHandler,
    on_drop: TabDrop,
    on_prepaint: TabPrepaint,
    collapsed: bool,
    show_guidance: bool,
    show_drag_label: bool,
) -> impl IntoElement {
    let dismiss = icon_button(
        "sidebar-favourites-hint-dismiss",
        "Dismiss favourites hint",
        true,
        metrics::ICON_BUTTON_SIZE,
        close_icon(palette.text_secondary, metrics::CHIP_CLOSE_ICON_SIZE),
        palette,
        on_dismiss,
    );
    v_stack()
        .id("sidebar-favourites-empty")
        .owns_clicks()
        .role(Role::Group)
        .aria_label("Add a favourite")
        .relative()
        .when(collapsed, |hint| {
            hint.size(px(metrics::SIDEBAR_COLLAPSED_ITEM_SIZE))
                .items_center()
                .justify_center()
        })
        .when(!collapsed, |hint| {
            hint.w_full()
                .items_center()
                .gap(px(metrics::SIDEBAR_FAVOURITES_HINT_GAP))
                .p(px(metrics::SIDEBAR_FAVOURITES_HINT_PADDING))
        })
        .when(!collapsed && !show_drag_label, |hint| {
            hint.h(px(metrics::SIDEBAR_FAVOURITES_HINT_BADGE_SIZE
                + 2.0 * metrics::SIDEBAR_FAVOURITES_HINT_PADDING))
                .justify_center()
        })
        .rounded(px(metrics::sidebar_item_radius()))
        .border_1()
        .border_dashed()
        .border_dashed_length(metrics::SIDEBAR_FAVOURITES_HINT_DASH_LENGTH)
        .border_dashed_gap(metrics::SIDEBAR_FAVOURITES_HINT_DASH_GAP)
        .border_color(rgba(palette.soft_accent_border))
        .bg(rgba(palette.surface))
        .on_drag_over::<DraggedTab>(move |style, _, _, _| {
            style
                .bg(rgba(palette.hover_surface))
                .border_color(rgba(palette.chosen))
        })
        .on_drop(move |dragged: &DraggedTab, window, cx| on_drop(dragged, window, cx))
        .on_prepaint(move |prepaint, window, cx| on_prepaint(prepaint.bounds, window, cx))
        .when(collapsed, |hint| {
            hint.child(favourite_star_icon(
                palette.chosen,
                metrics::SIDEBAR_FAVOURITES_HINT_ICON_SIZE,
            ))
        })
        .when(!collapsed, |hint| {
            hint.child(
                div()
                    .size(px(metrics::SIDEBAR_FAVOURITES_HINT_BADGE_SIZE))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(metrics::SIDEBAR_FAVOURITES_HINT_BADGE_SIZE / 2.0))
                    .bg(rgba(palette.chosen))
                    .child(favourite_star_icon(
                        palette.on_chosen,
                        metrics::SIDEBAR_FAVOURITES_HINT_ICON_SIZE,
                    )),
            )
        })
        .when(!collapsed && show_drag_label, |hint| {
            hint.child(
                div()
                    .text_size(px(metrics::SIDEBAR_FAVOURITES_HINT_TITLE_SIZE))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(palette.text_primary))
                    .text_center()
                    .child("Drag to add favourites"),
            )
        })
        .when(!collapsed && show_guidance, |hint| {
            hint.child(
                div()
                    .text_size(px(metrics::SIDEBAR_FAVOURITES_HINT_BODY_SIZE))
                    .text_color(rgb(palette.text_secondary))
                    .text_center()
                    .child("Or right-click a tab to add it."),
            )
        })
        .when(!collapsed && show_guidance, |hint| {
            hint.child(
                div()
                    .absolute()
                    .top(px(metrics::SIDEBAR_FAVOURITES_HINT_DISMISS_INSET))
                    .right(px(metrics::SIDEBAR_FAVOURITES_HINT_DISMISS_INSET))
                    .child(dismiss),
            )
        })
}

fn favourite_tile(
    index: usize,
    tile: FavouriteTile,
    collapsed: bool,
    suppress_tooltips: bool,
    palette: ThemeColors,
) -> AnyElement {
    let tooltip =
        (collapsed && !suppress_tooltips).then(|| label_tooltip(tile.label.clone(), palette));
    let tile_element = h_stack()
        .id(("sidebar-favourite", index))
        .role(Role::Button)
        .aria_label(tile.label)
        .aria_selected(tile.active)
        .tab_index(0)
        .focus_ring(palette)
        .items_center()
        .justify_center()
        // At rest, under the pointer and open, in rising strength, as tabs.
        .bg(rgba(if tile.active {
            palette.selected_surface
        } else {
            palette.surface
        }))
        .when(!tile.active, |tile| {
            tile.hover_style(|style| style.bg(rgba(palette.surface_hover)))
                .active_style(|style| style.bg(rgba(palette.selected_surface)))
        })
        .owns_clicks()
        .pressable()
        .on_mouse_down(MouseButton::Right, tile.on_context_menu)
        .on_click(tile.on_open)
        .child(tile.icon)
        .when_some(tooltip, |tile, tooltip| tile.tooltip(tooltip));
    if collapsed {
        return collapsed_tile(tile_element).into_any_element();
    }
    tile_element
        .flex_1()
        .h(px(metrics::SIDEBAR_FAVOURITE_HEIGHT))
        .rounded(px(metrics::sidebar_item_radius()))
        .into_any_element()
}
