//! Builds the window's chrome from its state: the horizontal tab strip and
//! address toolbar, or the sidebar (navigation, address field, favourites,
//! tabs and footer), shown beside the page or revealed over it.

use gpui::{AnyElement, ClickEvent, Context, MouseDownEvent, Window, point, prelude::*, px, rgba};
use photon_core::{BrowserCommand, TabLayout};

use super::super::layout::{Elevated, Elevation, Raised};
use super::super::motion::{AnimateIn, Edge, Entrance};
use super::super::pages::SETTINGS;
use super::super::settings::Settings;
use super::super::sidebar::{
    FooterActions, NavigationActions, NavigationState, SidebarSections, favourites_grid, footer,
    navigation_bar, sidebar,
};
use super::super::tabs::{DraggedTab, tab_list, tab_strip};
use super::super::titlebar::titlebar;
use super::super::toolbar::address_toolbar;
use super::super::{metrics, theme::ThemeColors};
use super::BrowserWindow;
use super::menu::OpenMenu;

impl BrowserWindow {
    /// The chrome for the chosen tab layout.
    pub(super) fn render_chrome(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        match Settings::get(cx).tab_layout {
            TabLayout::Horizontal => self.render_tab_strip(window, cx),
            TabLayout::Vertical => self.render_sidebar(window, cx),
        }
    }

    /// The tab strip in the titlebar, over the address toolbar.
    fn render_tab_strip(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette(window, cx);
        let strip = tab_strip(self.tab_items(cx), self.new_tab_handler(cx), palette);
        let toolbar = address_toolbar(
            self.omnibox.clone(),
            self.navigation_state(cx),
            self.navigation_actions(cx),
            matches!(self.open_menu, Some(OpenMenu::Toolbar(_))),
            Box::new(cx.listener(|this, event: &ClickEvent, _, cx| {
                cx.stop_propagation();
                this.open_menu = match this.open_menu {
                    Some(OpenMenu::Toolbar(_)) => None,
                    _ => Some(OpenMenu::Toolbar(toolbar_menu_anchor(event))),
                };
                cx.notify();
            })),
            palette,
        );
        gpui::div()
            .w_full()
            .flex()
            .flex_col()
            .child(titlebar(strip, window).on_mouse_down(
                gpui::MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.open_menu = Some(OpenMenu::Context(event.position));
                    cx.notify();
                }),
            ))
            .child(toolbar)
            .into_any_element()
    }

    /// Opens the browser menu on a right-click in empty titlebar space.
    pub(super) fn with_titlebar_menu(&self, bar: gpui::Div, cx: &mut Context<Self>) -> gpui::Div {
        bar.on_mouse_down(
            gpui::MouseButton::Right,
            cx.listener(|this, event: &MouseDownEvent, _, cx| {
                cx.stop_propagation();
                this.open_menu = Some(OpenMenu::Context(event.position));
                cx.notify();
            }),
        )
    }

    /// The sidebar as a layer over the window: its navigation row, which
    /// stays put, and the rest, which slides in from the left edge as it
    /// shows or is revealed. Shown, the page narrows beside it in step;
    /// revealed, it sits on a raised surface over the page.
    fn render_sidebar(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette(window, cx);
        let shown = self.sidebar.shown();
        let out = shown.max(self.sidebar.revealed());
        let width = self.sidebar.width(cx);
        let navigation = self.with_titlebar_menu(
            navigation_bar(
                self.navigation_state(cx),
                width,
                out,
                self.sidebar.visible,
                window.is_fullscreen() || window.is_simple_fullscreen(),
                self.navigation_actions(cx),
                palette,
            ),
            cx,
        );
        let mut layer = gpui::div().size_full().relative();
        if out > 0.0 {
            let mut panel = gpui::div()
                .id("sidebar-panel")
                .absolute()
                .top_0()
                .bottom_0()
                .left(px((out - 1.0) * width))
                .w(px(width))
                .occlude()
                // Two fingers moving sideways switch space.
                .on_scroll_wheel(cx.listener(|this, event, _, cx| this.swipe_spaces(event, cx)));
            if self.sidebar.over_page() {
                // Over the page it needs a surface, which fades as the page
                // makes room for it when it is kept shown.
                panel = panel
                    .child(
                        gpui::div()
                            .absolute()
                            .inset_0()
                            .raised(palette)
                            .border_r_1()
                            .border_color(rgba(palette.menu_border))
                            .elevated(Elevation::High)
                            .opacity(1.0 - shown),
                    )
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        this.revealed_sidebar_hovered(*hovered, cx);
                    }));
            }
            layer = layer.child(
                panel.child(
                    gpui::div()
                        .relative()
                        .size_full()
                        .pt(px(metrics::TITLEBAR_HEIGHT))
                        .child(self.sidebar_body(palette, cx)),
                ),
            );
        }
        // Kept shown, its edge can be dragged to resize it.
        if self.sidebar.visible && !self.sidebar.over_page() {
            layer = layer.child(self.resize_handle(cx));
        }
        layer
            .child(gpui::div().absolute().top_0().left_0().child(navigation))
            .into_any_element()
    }

    /// Everything in the sidebar below its navigation row.
    fn sidebar_body(&self, palette: ThemeColors, cx: &mut Context<Self>) -> impl IntoElement {
        let sidebar_settings = Settings::get(cx).sidebar.clone();
        let favourites = self.favourite_tiles(&sidebar_settings.favourites, palette, cx);
        let space = self.current_space(cx);
        // The space's own tabs; a favourite's tab shows as its tile instead.
        let tabs = tab_list(
            self.tab_items(cx)
                .into_iter()
                .filter(|item| {
                    let tab = &self.tabs[item.index];
                    tab.space == space && tab.favourite.is_none()
                })
                .collect(),
            // As in Arc, a new tab starts from the command bar.
            Box::new(cx.listener(|this, _, window, cx| {
                cx.stop_propagation();
                this.open_command_bar(window, cx);
            })),
            Box::new(cx.listener(|this, dragged: &DraggedTab, _, cx| {
                this.drop_tab_into_today(dragged.index, cx);
            })),
            palette,
        );
        // Just after switching space, its tabs slide in from that side.
        let arriving = self
            .spaces
            .arriving_from(Entrance::slide_in(Edge::Left).total_duration());
        let tabs = match arriving {
            Some(edge) => gpui::div()
                .w_full()
                .child(tabs)
                .animate_in(("space-tabs", space as usize), Entrance::slide_in(edge))
                .into_any_element(),
            None => tabs.into_any_element(),
        };
        sidebar(
            SidebarSections {
                address: self.omnibox.clone().into_any_element(),
                favourites: (!favourites.is_empty())
                    .then(|| favourites_grid(favourites, palette).into_any_element()),
                tabs,
                footer: footer(
                    matches!(self.open_menu, Some(OpenMenu::Sidebar(_))),
                    self.space_dots(palette, cx),
                    FooterActions {
                        settings: Box::new(cx.listener(|this, _, window, cx| {
                            this.open_page_tab(SETTINGS, window, cx);
                        })),
                        menu: Box::new(cx.listener(|this, event: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            this.open_menu = match this.open_menu {
                                Some(OpenMenu::Sidebar(_)) => None,
                                _ => Some(OpenMenu::Sidebar(footer_menu_anchor(event))),
                            };
                            cx.notify();
                        })),
                        new_space: Box::new(cx.listener(|this, _, window, cx| {
                            this.edit_space(None, window, cx);
                        })),
                    },
                    palette,
                )
                .into_any_element(),
            },
            palette,
        )
    }

    fn new_tab_handler(&self, cx: &mut Context<Self>) -> super::super::ClickHandler {
        Box::new(cx.listener(|this, _, window, cx| {
            cx.stop_propagation();
            this.open_tab(window, cx);
        }))
    }

    fn navigation_state(&self, cx: &Context<Self>) -> NavigationState {
        self.active_webview()
            .map_or_else(NavigationState::default, |webview| {
                let view = webview.read(cx);
                NavigationState {
                    can_go_back: view.state.can_go_back,
                    can_go_forward: view.state.can_go_forward,
                    can_reload: view.has_page(),
                    loading: view.state.loading,
                }
            })
    }

    fn navigation_actions(&self, cx: &mut Context<Self>) -> NavigationActions {
        let loading = self.navigation_state(cx).loading;
        NavigationActions {
            toggle_sidebar: Box::new(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx))),
            back: Box::new(cx.listener(|this, _, window, cx| {
                this.dispatch_command(BrowserCommand::Back, window, cx);
            })),
            forward: Box::new(cx.listener(|this, _, window, cx| {
                this.dispatch_command(BrowserCommand::Forward, window, cx);
            })),
            reload: Box::new(cx.listener(move |this, _, window, cx| {
                let command = if loading {
                    BrowserCommand::StopLoading
                } else {
                    BrowserCommand::Reload
                };
                this.dispatch_command(command, window, cx);
            })),
        }
    }
}

/// Anchors the toolbar's menu below the menu button's bottom-right corner.
fn toolbar_menu_anchor(event: &ClickEvent) -> gpui::Point<gpui::Pixels> {
    match event {
        ClickEvent::Keyboard(event) => event.bounds.bottom_right(),
        ClickEvent::Mouse(_) | ClickEvent::Touch(_) => {
            let position = event.position();
            let half_button = px(metrics::TOOLBAR_BUTTON_SIZE / 2.0);
            point(position.x + half_button, position.y + half_button)
        }
    }
}

/// Anchors the footer's menu above the menu button.
fn footer_menu_anchor(event: &ClickEvent) -> gpui::Point<gpui::Pixels> {
    match event {
        ClickEvent::Keyboard(event) => event.bounds.origin,
        ClickEvent::Mouse(_) | ClickEvent::Touch(_) => {
            let position = event.position();
            let half_button = px(metrics::TOOLBAR_BUTTON_SIZE / 2.0);
            point(position.x + half_button, position.y - half_button)
        }
    }
}
