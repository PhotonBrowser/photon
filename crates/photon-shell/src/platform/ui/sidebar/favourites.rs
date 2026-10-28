//! The grid of favourite sites at the top of the sidebar.

use gpui::{
    AnyElement, App, ElementId, MouseButton, Role, SharedString, Window, div, prelude::*, px, rgb,
    rgba,
};

use super::super::animated_icons::{animated_icon, dismiss_x};
use super::super::button::icon_button;
use super::super::controls::FocusRing;
use super::super::controls::{OwnsClicks, Pressable};
use super::super::layout::{h_stack, v_stack};
use super::super::motion::{AnimateIn, Curve, Entrance, MotionConfig, Speed};
use super::super::tabs::{DraggedTab, TabDrop, TabPrepaint};
use super::super::tooltip::label_tooltip;
use super::super::{ClickHandler, MouseDownHandler, metrics, theme::ThemeColors};
use super::collapsed_tile;

/// A favourite site's tile.
pub(in super::super) struct FavouriteTile {
    /// The page the favourite opens, which also keys its tile, so tiles keep
    /// their state as others come and go.
    pub(in super::super) url: String,
    /// Whether the favourite was just added, so its tile scales in.
    pub(in super::super) appearing: bool,
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
/// to icons, a column of square tiles. While a tab is dragged over them, a
/// slot shows where it will land.
#[expect(
    clippy::too_many_arguments,
    reason = "the grid needs its drop state, handlers and motion settings"
)]
pub(in super::super) fn favourites_grid(
    tiles: Vec<FavouriteTile>,
    collapsed: bool,
    palette: ThemeColors,
    drop_hover: bool,
    has_room: bool,
    on_drop: TabDrop,
    on_prepaint: TabPrepaint,
    suppress_tooltips: bool,
    motion: &MotionConfig,
) -> impl IntoElement {
    let columns = if collapsed {
        1
    } else {
        metrics::SIDEBAR_FAVOURITE_COLUMNS
    };
    let gap = metrics::SidebarSpacing::new(collapsed).favourite_gap;
    let mut cells: Vec<AnyElement> = tiles
        .into_iter()
        .map(|tile| favourite_tile(tile, collapsed, suppress_tooltips, palette, motion))
        .collect();
    if drop_hover && has_room {
        cells.push(drop_slot(collapsed, palette, motion));
    }
    let mut rows = Vec::new();
    let mut cells = cells.into_iter().peekable();
    while cells.peek().is_some() {
        let row: Vec<AnyElement> = cells.by_ref().take(columns).collect();
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
        .on_drop(move |dragged: &DraggedTab, window, cx| on_drop(dragged, window, cx))
        .on_prepaint(move |prepaint, window, cx| on_prepaint(prepaint.bounds, window, cx))
        .children(rows)
}

/// How a favourite's tile grows in when it is added.
const fn favourite_entrance() -> Entrance {
    Entrance::new()
        .fade_from(0.0)
        .scale_from(metrics::SIDEBAR_FAVOURITE_ENTRANCE_SCALE)
        .curve(Curve::Overshoot)
        .speed(Speed::Standard)
}

/// The empty tile a dragged tab will fill, outlined as a drop target.
fn drop_slot(collapsed: bool, palette: ThemeColors, motion: &MotionConfig) -> AnyElement {
    let slot = div()
        .rounded(px(metrics::sidebar_item_radius()))
        .border_2()
        .border_dashed()
        .border_dashed_length(metrics::SIDEBAR_FAVOURITES_HINT_DASH_LENGTH)
        .border_dashed_gap(metrics::SIDEBAR_FAVOURITES_HINT_DASH_GAP)
        .border_color(rgba(palette.soft_accent_border))
        .bg(rgba(palette.surface_hover));
    let slot = if collapsed {
        collapsed_tile(slot)
    } else {
        slot.flex_1().h(px(metrics::SIDEBAR_FAVOURITE_HEIGHT))
    };
    slot.animate_in("sidebar-favourite-drop-slot", favourite_entrance(), motion)
}

/// Empty favourites area, which teaches new users to menu-favourite a tab.
#[expect(
    clippy::too_many_arguments,
    reason = "the hint needs its drop and prepaint handlers and the window to animate its icons"
)]
pub(in super::super) fn favourites_empty_state(
    palette: ThemeColors,
    on_dismiss: ClickHandler,
    on_drop: TabDrop,
    on_prepaint: TabPrepaint,
    collapsed: bool,
    show_guidance: bool,
    show_drag_label: bool,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let dismiss = icon_button(
        "sidebar-favourites-hint-dismiss",
        "Dismiss favourites hint",
        true,
        metrics::ICON_BUTTON_SIZE,
        animated_icon(
            "sidebar-favourites-hint-dismiss",
            dismiss_x(),
            palette.text_secondary,
            metrics::CHIP_CLOSE_ICON_SIZE,
            window,
            cx,
        )
        .size_full(),
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
        .rounded(px(metrics::SIDEBAR_FAVOURITES_HINT_RADIUS))
        .border_2()
        .border_dashed()
        .border_dashed_length(metrics::SIDEBAR_FAVOURITES_HINT_DASH_LENGTH)
        .border_dashed_gap(metrics::SIDEBAR_FAVOURITES_HINT_DASH_GAP)
        .border_color(rgb(palette.text_secondary))
        .on_drop(move |dragged: &DraggedTab, window, cx| on_drop(dragged, window, cx))
        .on_prepaint(move |prepaint, window, cx| on_prepaint(prepaint.bounds, window, cx))
        .when(!collapsed && show_drag_label, |hint| {
            hint.child(
                div()
                    .text_size(px(metrics::SIDEBAR_FAVOURITES_HINT_TITLE_SIZE))
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
    tile: FavouriteTile,
    collapsed: bool,
    suppress_tooltips: bool,
    palette: ThemeColors,
    motion: &MotionConfig,
) -> AnyElement {
    let id = ElementId::from(SharedString::from(format!(
        "sidebar-favourite-{}",
        tile.url
    )));
    let tooltip =
        (collapsed && !suppress_tooltips).then(|| label_tooltip(tile.label.clone(), palette));
    let tile_element = h_stack()
        .id(id.clone())
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
    let tile_element = if collapsed {
        collapsed_tile(tile_element)
    } else {
        tile_element
            .flex_1()
            .h(px(metrics::SIDEBAR_FAVOURITE_HEIGHT))
            .rounded(px(metrics::sidebar_item_radius()))
    };
    if tile.appearing {
        tile_element.animate_in((id, "entrance"), favourite_entrance(), motion)
    } else {
        tile_element.into_any_element()
    }
}
