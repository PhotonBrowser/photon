//! The new tab page's shortcut tiles.

use gpui::{
    AnyElement, Context, ElementId, ImageSource, MouseButton, ObjectFit, Role, SharedString, div,
    img, prelude::*, px, rgb, rgba,
};
use photon_core::{Shortcut, site_name};

use super::super::super::button::icon_button;
use super::super::super::history::BrowsingHistory;
use super::super::super::icons::{add_icon, close_icon, globe_icon};
use super::super::super::layout::{h_stack, v_stack};
use super::super::super::motion::{AnimateIn, Entrance};
use super::super::super::settings::Settings;
use super::super::super::{metrics, theme::ThemeColors};
use super::NewTabPage;

impl NewTabPage {
    /// The tiles in at most two rows, then a tile for adding one while there
    /// is room.
    pub(super) fn shortcut_grid(
        &self,
        shortcuts: Vec<Shortcut>,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let layout = &Settings::get(cx).new_tab;
        let (limit, columns) = (layout.shortcut_limit(), layout.shortcut_columns());
        let room_left = shortcuts.len() < limit;
        let tiles: Vec<AnyElement> = shortcuts
            .into_iter()
            .enumerate()
            .map(|(index, shortcut)| self.shortcut_tile(index, shortcut, palette, cx))
            .collect();
        let add = room_left.then(|| {
            tile(
                "new-tab-add-shortcut",
                "Add shortcut",
                add_icon(palette.text_secondary, metrics::SHORTCUT_ICON_SIZE),
                palette,
            )
            .on_click(cx.listener(|this, _, window, cx| this.start_adding(window, cx)))
        });
        h_stack()
            .id("new-tab-shortcuts")
            .role(Role::List)
            .aria_label("Shortcuts")
            .flex_wrap()
            .justify_center()
            .max_w(px(
                columns as f32 * (metrics::SHORTCUT_TILE_WIDTH + metrics::SHORTCUT_GAP)
            ))
            .gap(px(metrics::SHORTCUT_GAP))
            .children(tiles)
            .children(add)
    }

    fn shortcut_tile(
        &self,
        index: usize,
        shortcut: Shortcut,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let icon = match BrowsingHistory::favicon(&shortcut.url, cx) {
            Some(favicon) => img(ImageSource::Render(favicon.image))
                .size(px(metrics::SHORTCUT_ICON_SIZE))
                .object_fit(ObjectFit::Contain)
                .into_any_element(),
            None => {
                globe_icon(palette.text_secondary, metrics::SHORTCUT_ICON_SIZE).into_any_element()
            }
        };
        // Your own shortcuts keep the name you gave; visited sites go by their
        // address, as page titles are often long.
        let pinned = Settings::get(cx).new_tab.is_pinned(&shortcut.url);
        let name = if pinned && !shortcut.title.trim().is_empty() {
            shortcut.title.clone()
        } else {
            site_name(&shortcut.url)
        };
        let url = shortcut.url.clone();
        let mut tile =
            tile(("new-tab-shortcut", index), name, icon, palette)
                .on_click(cx.listener(move |this, _, _, cx| this.context.open(url.clone(), cx)))
                .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                    this.hover_tile(index, *hovered, cx)
                }));
        if self.hovered == Some(index) {
            let url = shortcut.url;
            tile = tile.child(
                div()
                    .absolute()
                    .top(px(metrics::SHORTCUT_REMOVE_INSET))
                    .right(px(metrics::SHORTCUT_REMOVE_INSET))
                    .child(icon_button(
                        "new-tab-remove-shortcut",
                        "Remove shortcut",
                        true,
                        metrics::CHIP_CLOSE_SIZE,
                        close_icon(palette.text_secondary, metrics::CHIP_CLOSE_ICON_SIZE),
                        palette,
                        Box::new(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.remove(&url, cx);
                        })),
                    ))
                    .animate_in(("new-tab-remove-shortcut", index), Entrance::fade()),
            );
        }
        tile.into_any_element()
    }
}

/// A tile with a round icon over a name.
fn tile(
    id: impl Into<ElementId>,
    name: impl Into<SharedString>,
    icon: impl IntoElement,
    palette: ThemeColors,
) -> gpui::Stateful<gpui::Div> {
    let name = name.into();
    v_stack()
        .id(id)
        .role(Role::ListItem)
        .aria_label(name.clone())
        .tab_index(0)
        .focus_visible(|style| style.border_1().border_color(rgb(palette.chosen)))
        .relative()
        .items_center()
        .gap(px(metrics::SHORTCUT_GAP))
        .w(px(metrics::SHORTCUT_TILE_WIDTH))
        .py(px(metrics::INTERNAL_PAGE_GROUP_GAP))
        .px(px(metrics::SHORTCUT_GAP))
        .rounded(px(metrics::SURFACE_RADIUS))
        .hover(|style| style.bg(rgba(palette.hover_surface)))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            h_stack()
                .size(px(metrics::SHORTCUT_ICON_BOX_SIZE))
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(rgba(palette.menu_surface))
                .child(icon),
        )
        .child(
            div()
                .w_full()
                .text_center()
                .truncate()
                .text_size(px(metrics::TAB_FONT_SIZE))
                .text_color(rgb(palette.text_primary))
                .child(name),
        )
}
