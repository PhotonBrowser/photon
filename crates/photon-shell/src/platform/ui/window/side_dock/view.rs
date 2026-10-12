//! Draws the sidebar: its navigation at the top and the workspace's panels
//! below, with labels or collapsed to icons. It slides beside the page or
//! over it, and its content stays put while its width changes, fading
//! between icons and labels.

use gpui::{AnyElement, Context, Div, Stateful, Window, div, prelude::*, px, rgba};
use photon_core::DockRegion;

use super::super::super::button::toolbar_button;
use super::super::super::icons::{sidebar_expand_icon, sidebar_icon};
use super::super::super::layout::{Elevated, Elevation, Raised, v_stack};
use super::super::super::motion::{AnimateIn, Edge, Entrance};
use super::super::super::pages::SETTINGS;
use super::super::super::settings::Settings;
use super::super::super::sidebar::{
    FooterActions, SidebarSection, collapsed_tile, favourites_empty_state, favourites_grid, footer,
    navigation_bar, navigation_column, sidebar,
};
use super::super::super::tabs::{TabDragMove, tab_list};
use super::super::super::titlebar::window_drag_area;
use super::super::super::tooltip::label_tooltip;
use super::super::super::workspace_layout::{ComponentId, WorkspaceLayout};
use super::super::super::{ClickHandler, metrics, theme::ThemeColors};
use super::super::BrowserWindow;

impl BrowserWindow {
    /// The sidebar in `region`, while it is shown or revealed, and the
    /// handle for resizing it while it is beside the page.
    pub(in super::super) fn render_sidebar(
        &self,
        region: DockRegion,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let sidebar = &self.sidebar;
        let out = sidebar.shown().max(sidebar.revealed());
        let mut layer = div().size_full().relative();
        if out > 0.0 {
            layer = layer.child(self.sidebar_panel(region, out, window, cx));
        }
        if sidebar.docked() && !sidebar.over_page() {
            layer = layer.child(self.resize_handle(region, cx));
        }
        layer.into_any_element()
    }

    /// The panel `out` of the way onto the window, clipping its content to
    /// the width it has now.
    fn sidebar_panel(
        &self,
        region: DockRegion,
        out: f32,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let palette = self.palette(window, cx);
        let layout = WorkspaceLayout::current(cx);
        let sidebar = &self.sidebar;
        let leading = region == DockRegion::Leading;
        let over_page = sidebar.over_page();
        let collapsed = sidebar.shows_icons();
        let full_width = sidebar.width(cx);
        let width = if over_page {
            full_width
        } else {
            sidebar.docked_width(cx)
        };
        let (content_width, opacity) = if collapsed {
            (metrics::SIDEBAR_COLLAPSED_WIDTH, 1.0 - sidebar.expanded())
        } else if over_page {
            (full_width, 1.0)
        } else {
            (full_width, sidebar.expanded())
        };
        // Right-clicking its empty space opens the browser menu.
        let content = v_stack()
            .absolute()
            .top_0()
            .bottom_0()
            .when(leading, |content| content.left_0())
            .when(!leading, |content| content.right_0())
            .w(px(content_width))
            .opacity(opacity)
            .child(self.sidebar_navigation(region, full_width, palette, window, cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(self.sidebar_body(region, collapsed, palette, cx)),
            );
        let content = self.with_browser_menu(content, cx);

        let chrome_shown = sidebar.chrome_shown();
        let offset = px((out - 1.0) * width);
        let reveal_inset = if over_page {
            metrics::SIDEBAR_REVEAL_INSET
        } else {
            0.0
        };
        let mut panel = window_drag_area(
            div()
                .id("sidebar-panel")
                .absolute()
                .top(px(layout.top_extent() * chrome_shown + reveal_inset))
                .bottom(px(layout.bottom_extent() * chrome_shown + reveal_inset))
                .w(px(width))
                .when(over_page, |panel| {
                    panel.rounded(px(metrics::SIDEBAR_REVEAL_RADIUS))
                })
                .overflow_hidden()
                .occlude()
                .on_scroll_wheel(cx.listener(|this, event, _, cx| this.swipe_spaces(event, cx))),
        );
        panel = if leading {
            panel.left(offset + px(reveal_inset))
        } else {
            panel.right(offset + px(reveal_inset))
        };
        if over_page {
            panel = panel
                .child(revealed_surface(palette).opacity(1.0 - sidebar.shown()))
                .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                    this.revealed_sidebar_hovered(*hovered, cx);
                }));
        }
        panel.child(content)
    }

    /// The sidebar's top: a row beside the window controls with the toggle
    /// and navigation, or collapsed, a column with only the toggle.
    fn sidebar_navigation(
        &self,
        region: DockRegion,
        width: f32,
        palette: ThemeColors,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let layout = WorkspaceLayout::current(cx);
        let leading = region == DockRegion::Leading;
        let fullscreen = window.is_fullscreen() || window.is_simple_fullscreen();
        let has_toggle = layout
            .components(region)
            .any(|component| component.id == ComponentId::SidebarToggle);
        if self.sidebar.shows_icons() {
            let toggle = has_toggle.then(|| {
                collapsed_tile(self.sidebar_toggle(
                    metrics::SIDEBAR_COLLAPSED_ICON_SIZE,
                    palette,
                    cx,
                ))
                .tooltip(label_tooltip(self.sidebar_toggle_label(), palette))
                .into_any_element()
            });
            navigation_column(fullscreen, leading, toggle)
        } else {
            let toggle = has_toggle.then(|| {
                self.sidebar_toggle(metrics::TOOLBAR_ICON_SIZE, palette, cx)
                    .into_any_element()
            });
            let controls = layout
                .components(region)
                .filter(|component| component.id.is_navigation_control())
                .map(|component| self.render_component(component.id, component.region, window, cx))
                .collect();
            navigation_bar(width, fullscreen, leading, toggle, controls)
        }
    }

    /// The sidebar's button, labelled for what pressing it does now.
    pub(in super::super) fn sidebar_toggle(
        &self,
        icon_size: f32,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let sidebar = &self.sidebar;
        let color = palette.text_secondary;
        let label = self.sidebar_toggle_label();
        let icon = if sidebar.docked() && sidebar.shows_icons() {
            sidebar_expand_icon(color, icon_size).into_any_element()
        } else {
            sidebar_icon(color, icon_size).into_any_element()
        };
        toolbar_button(
            "toolbar-sidebar",
            label,
            0,
            true,
            icon,
            palette,
            Box::new(cx.listener(|this, _, _, cx| this.press_sidebar_toggle(cx))),
        )
    }

    fn sidebar_toggle_label(&self) -> &'static str {
        let sidebar = &self.sidebar;
        if sidebar.over_page() {
            "Keep sidebar open"
        } else if !sidebar.docked() {
            "Show sidebar"
        } else if sidebar.collapsed() {
            "Expand sidebar"
        } else {
            "Hide sidebar"
        }
    }

    /// The panels the workspace places in the sidebar, in its order.
    /// Collapsed to icons, it leaves out the address field and the hint for
    /// adding favourites, which need the room.
    fn sidebar_body(
        &self,
        region: DockRegion,
        collapsed: bool,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let layout = WorkspaceLayout::current(cx);
        let sections = layout
            .components(region)
            .filter_map(|component| match component.id {
                ComponentId::AddressField if !collapsed => Some(SidebarSection::Address(
                    self.omnibox.clone().into_any_element(),
                )),
                ComponentId::FavouritesPanel => self
                    .sidebar_favourites(collapsed, palette, cx)
                    .map(SidebarSection::Favourites),
                ComponentId::VerticalTabs => Some(SidebarSection::Tabs(
                    self.sidebar_tabs(collapsed, palette, cx),
                )),
                ComponentId::SidebarFooter => Some(SidebarSection::Footer(
                    self.sidebar_footer(collapsed, palette, cx),
                )),
                _ => None,
            })
            .collect();
        sidebar(sections, collapsed, palette)
    }

    /// The space's favourites, or with none yet, the hint for adding one
    /// until it is dismissed.
    fn sidebar_favourites(
        &self,
        collapsed: bool,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let space = Settings::get(cx).spaces.active();
        let (space_id, saved, hint_dismissed) = (
            space.id,
            space.favourites.clone(),
            space.favourites_hint_dismissed,
        );
        let tiles = self.favourite_tiles(&saved, collapsed, palette, cx);
        if !tiles.is_empty() {
            return Some(
                favourites_grid(
                    tiles,
                    collapsed,
                    palette,
                    self.favourite_drop_handler(cx),
                    self.favourite_bounds_handler(cx),
                    self.tab_drag_active(),
                )
                .into_any_element(),
            );
        }
        let dragging_over_favourites = self.favourites_drag_hover.is_some();
        if !should_show_empty_favourites_hint(hint_dismissed, collapsed, dragging_over_favourites) {
            return None;
        }
        let on_dismiss: ClickHandler = Box::new(cx.listener(move |_, _, _, cx| {
            Settings::update(cx, |settings| {
                if let Some(space) = settings.spaces.get_mut(space_id) {
                    space.favourites_hint_dismissed = true;
                }
            });
        }));
        Some(
            favourites_empty_state(
                palette,
                on_dismiss,
                self.favourite_drop_handler(cx),
                self.favourite_bounds_handler(cx),
                collapsed,
                should_show_favourites_guidance(hint_dismissed),
            )
            .into_any_element(),
        )
    }

    /// The space's tabs, sliding in from the side the space came from.
    fn sidebar_tabs(
        &self,
        collapsed: bool,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let space = self.current_space(cx);
        let listed = self.listed_tabs(cx);
        let items = self
            .tab_items(cx)
            .into_iter()
            .filter(|item| listed.contains(&item.index))
            .collect();
        let browser = cx.entity().downgrade();
        let on_drag_move: TabDragMove =
            std::rc::Rc::new(move |event, phase, hitbox, window, cx| {
                browser
                    .update(cx, |this, cx| {
                        this.update_tab_drag(event, phase, hitbox, window, cx);
                    })
                    .ok();
            });
        let tabs = tab_list(
            items,
            Box::new(cx.listener(|this, _, window, cx| {
                cx.stop_propagation();
                this.open_command_bar(window, cx);
            })),
            self.drop_tab_into_today_handler(cx),
            on_drag_move,
            self.tab_drop_handler(cx),
            self.tab_container_bounds_handler(cx),
            collapsed,
            palette,
        );
        match self.spaces.arriving_from(
            Entrance::slide_in(Edge::Left).total_duration(&Settings::get(cx).animations),
        ) {
            Some(edge) => div()
                .w_full()
                .child(tabs)
                .animate_in(
                    ("space-tabs", space as usize),
                    Entrance::slide_in(edge),
                    &Settings::get(cx).animations,
                )
                .into_any_element(),
            None => tabs.into_any_element(),
        }
    }

    fn sidebar_footer(
        &self,
        collapsed: bool,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        footer(
            self.space_dots(palette, cx),
            FooterActions {
                settings: Box::new(cx.listener(|this, _, window, cx| {
                    this.open_page_tab(SETTINGS, window, cx);
                })),
                menu: Box::new(cx.listener(|this, _, window, cx| {
                    cx.stop_propagation();
                    this.show_browser_menu(window, cx);
                })),
                new_space: Box::new(cx.listener(|this, _, window, cx| {
                    this.edit_space(None, window, cx);
                })),
            },
            collapsed,
            palette,
        )
    }
}

/// The floating raised surface under a sidebar revealed over the page.
fn revealed_surface(palette: ThemeColors) -> Div {
    div()
        .absolute()
        .inset_0()
        .rounded(px(metrics::SIDEBAR_REVEAL_RADIUS))
        .border_1()
        .border_color(rgba(palette.menu_border))
        .raised(palette)
        .elevated(Elevation::High)
}

fn should_show_empty_favourites_hint(
    hint_dismissed: bool,
    collapsed: bool,
    dragging_over_favourites: bool,
) -> bool {
    !collapsed && (!hint_dismissed || dragging_over_favourites)
}

fn should_show_favourites_guidance(hint_dismissed: bool) -> bool {
    !hint_dismissed
}

#[cfg(test)]
mod tests {
    use super::{should_show_empty_favourites_hint, should_show_favourites_guidance};

    #[test]
    fn dismissed_hint_returns_as_a_compact_drop_target_during_a_favourites_drag_hover() {
        assert!(!should_show_empty_favourites_hint(true, false, false));
        assert!(should_show_empty_favourites_hint(true, false, true));
        assert!(should_show_empty_favourites_hint(false, false, false));
        assert!(!should_show_empty_favourites_hint(false, true, false));
    }

    #[test]
    fn drag_hover_does_not_restore_dismissed_favourites_guidance() {
        assert!(!should_show_favourites_guidance(true));
        assert!(should_show_favourites_guidance(false));
    }
}
