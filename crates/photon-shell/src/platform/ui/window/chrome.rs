//! Builds browser chrome from catalog components. Toolbar controls and side
//! panels are placed independently by the active workspace layout.

use gpui::{AnyElement, ClickEvent, Context, MouseDownEvent, Window, point, prelude::*, px, rgba};
use photon_core::{BrowserCommand, DockRegion};

use super::super::button::{menu_button, toolbar_button};
use super::super::icons::{back_icon, close_icon, forward_icon, reload_icon, sidebar_icon};
use super::super::layout::{Elevated, Elevation, Raised, v_stack};
use super::super::motion::{AnimateIn, Edge, Entrance};
use super::super::pages::SETTINGS;
use super::super::settings::Settings;
use super::super::sidebar::{
    FooterActions, SidebarSection, favourites_grid, footer, navigation_bar, sidebar,
};
use super::super::tabs::{DraggedTab, tab_list, tab_strip, tab_strip_content};
use super::super::titlebar::titlebar;
use super::super::titlebar::window_drag_area;
use super::super::toolbar::toolbar_row;
use super::super::workspace_layout::{ComponentId, WorkspaceLayout};
use super::super::{metrics, theme::ThemeColors};
use super::BrowserWindow;
use super::menu::OpenMenu;

impl BrowserWindow {
    /// Renders catalog components into the four docks around the page.
    pub(super) fn render_chrome(&mut self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let editor = self.workspace_chrome_presence.sync(
            self.workspace_edit.clone(),
            Entrance::new().fade_from(0.0),
            cx,
        );
        if let Some((editing, transition)) = editor {
            return self.workspace_editor_chrome(&editing, transition, window, cx);
        }
        let layout = self.workspace_layout(cx);
        let top_components = layout.components(DockRegion::Top).collect::<Vec<_>>();
        let has_top_tab_strip = top_components
            .iter()
            .any(|component| component.id == ComponentId::TabStrip);
        let has_top_controls = top_components
            .iter()
            .any(|component| component.id != ComponentId::TabStrip);
        let has_sidebar = layout.has_sidebar();
        let mut root = gpui::div().size_full().relative();

        if !top_components.is_empty() || !has_sidebar {
            let mut top = v_stack().w_full();
            if has_top_tab_strip {
                top = top.child(self.render_component(
                    ComponentId::TabStrip,
                    DockRegion::Top,
                    window,
                    cx,
                ));
            } else if !has_sidebar || has_top_controls {
                let blank_titlebar = self.with_titlebar_menu(
                    window_drag_area(
                        gpui::div()
                            .w_full()
                            .h(px(metrics::TITLEBAR_HEIGHT))
                            .flex_shrink_0(),
                    ),
                    cx,
                );
                top = top.child(blank_titlebar);
            }
            let controls = top_components
                .iter()
                .filter(|component| component.id != ComponentId::TabStrip)
                .map(|component| self.render_component(component.id, component.region, window, cx))
                .collect::<Vec<_>>();
            if !controls.is_empty() {
                top = top.child(toolbar_row(controls));
            }
            root = root.child(top.absolute().top_0().left_0().right_0());
        }

        let bottom_components = layout.components(DockRegion::Bottom).collect::<Vec<_>>();
        if !bottom_components.is_empty() {
            let mut bottom = v_stack().w_full();
            if bottom_components
                .iter()
                .any(|component| component.id == ComponentId::TabStrip)
            {
                bottom = bottom.child(self.render_component(
                    ComponentId::TabStrip,
                    DockRegion::Bottom,
                    window,
                    cx,
                ));
            }
            let controls = bottom_components
                .iter()
                .filter(|component| component.id != ComponentId::TabStrip)
                .map(|component| self.render_component(component.id, component.region, window, cx))
                .collect::<Vec<_>>();
            if !controls.is_empty() {
                bottom = bottom.child(toolbar_row(controls));
            }
            root = root.child(bottom.absolute().bottom_0().left_0().right_0());
        }

        if let Some(region) = layout.sidebar_region() {
            root = root.child(self.render_sidebar(region, window, cx));
        }
        root.into_any_element()
    }

    pub(super) fn render_component(
        &self,
        component: ComponentId,
        region: DockRegion,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if component == ComponentId::TabStrip {
            let palette = self.palette(window, cx);
            let strip = tab_strip(self.tab_items(cx), self.new_tab_handler(cx), palette);
            return if region == DockRegion::Top {
                self.with_titlebar_menu(titlebar(strip, window), cx)
                    .into_any_element()
            } else {
                strip.into_any_element()
            };
        }

        let palette = self.palette(window, cx);
        let (color, size) = (palette.text_secondary, metrics::TOOLBAR_ICON_SIZE);
        match component {
            ComponentId::BackButton => toolbar_button(
                "toolbar-back",
                "Go back",
                0,
                self.navigation_state(cx).can_go_back,
                back_icon(color, size),
                palette,
                Box::new(cx.listener(|this, _, window, cx| {
                    this.dispatch_command(BrowserCommand::Back, window, cx);
                })),
            )
            .into_any_element(),
            ComponentId::ForwardButton => toolbar_button(
                "toolbar-forward",
                "Go forward",
                1,
                self.navigation_state(cx).can_go_forward,
                forward_icon(color, size),
                palette,
                Box::new(cx.listener(|this, _, window, cx| {
                    this.dispatch_command(BrowserCommand::Forward, window, cx);
                })),
            )
            .into_any_element(),
            ComponentId::ReloadButton => {
                let state = self.navigation_state(cx);
                let (label, icon) = if state.loading {
                    ("Stop loading", close_icon(color, size).into_any_element())
                } else {
                    ("Reload page", reload_icon(color, size).into_any_element())
                };
                toolbar_button(
                    "toolbar-reload",
                    label,
                    2,
                    state.can_reload || state.loading,
                    icon,
                    palette,
                    Box::new(cx.listener(move |this, _, window, cx| {
                        let command = if state.loading {
                            BrowserCommand::StopLoading
                        } else {
                            BrowserCommand::Reload
                        };
                        this.dispatch_command(command, window, cx);
                    })),
                )
                .into_any_element()
            }
            ComponentId::AddressField => self.omnibox.clone().into_any_element(),
            ComponentId::BrowserMenuButton => menu_button(
                if region == DockRegion::Leading || region == DockRegion::Trailing {
                    "side-browser-menu"
                } else {
                    "toolbar-menu"
                },
                4,
                matches!(self.open_menu, Some(OpenMenu::Toolbar(_))),
                palette,
                Box::new(cx.listener(|this, event: &ClickEvent, _, cx| {
                    cx.stop_propagation();
                    this.open_menu = match this.open_menu {
                        Some(OpenMenu::Toolbar(_)) => None,
                        _ => Some(OpenMenu::Toolbar(toolbar_menu_anchor(event))),
                    };
                    cx.notify();
                })),
            )
            .into_any_element(),
            ComponentId::SidebarToggle => toolbar_button(
                "toolbar-sidebar",
                if self.sidebar.visible {
                    "Hide sidebar"
                } else {
                    "Show sidebar"
                },
                0,
                true,
                sidebar_icon(color, size),
                palette,
                Box::new(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx))),
            )
            .into_any_element(),
            ComponentId::TabStrip
            | ComponentId::FavouritesPanel
            | ComponentId::VerticalTabs
            | ComponentId::SidebarFooter => gpui::div().into_any_element(),
        }
    }

    /// Renders only the visible tabs, sized so edit mode can frame their
    /// controls without outlining the full-width titlebar.
    pub(super) fn render_tab_strip_content(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette(window, cx);
        tab_strip_content(self.tab_items(cx), self.new_tab_handler(cx), palette).into_any_element()
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

    /// The side dock: one fixed navigation row and an ordered panel stack.
    fn render_sidebar(
        &self,
        region: DockRegion,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = self.palette(window, cx);
        let layout = WorkspaceLayout::current(cx);
        let components = layout.components(region).collect::<Vec<_>>();
        let shown = self.sidebar.shown();
        let out = shown.max(self.sidebar.revealed());
        let width = self.sidebar.width(cx);
        let top_extent = layout.top_extent();
        let bottom_extent = layout.bottom_extent();
        let leading = region == DockRegion::Leading;
        let toggle = components
            .iter()
            .find(|component| component.id == ComponentId::SidebarToggle)
            .map(|component| self.render_component(component.id, component.region, window, cx));
        let controls = components
            .iter()
            .filter(|component| {
                matches!(
                    component.id,
                    ComponentId::BackButton
                        | ComponentId::ForwardButton
                        | ComponentId::ReloadButton
                        | ComponentId::BrowserMenuButton
                )
            })
            .map(|component| self.render_component(component.id, component.region, window, cx))
            .collect::<Vec<_>>();
        let navigation = self.with_titlebar_menu(
            navigation_bar(
                width,
                out,
                window.is_fullscreen() || window.is_simple_fullscreen(),
                leading,
                toggle,
                controls,
            ),
            cx,
        );

        let mut layer = gpui::div().size_full().relative();
        if out > 0.0 {
            let mut panel = gpui::div()
                .id("sidebar-panel")
                .absolute()
                .top(px(top_extent))
                .bottom(px(bottom_extent))
                .w(px(width))
                .occlude()
                .on_scroll_wheel(cx.listener(|this, event, _, cx| this.swipe_spaces(event, cx)));
            panel = if leading {
                panel.left(px((out - 1.0) * width))
            } else {
                panel.right(px((out - 1.0) * width))
            };
            if self.sidebar.over_page() {
                let surface = gpui::div()
                    .absolute()
                    .inset_0()
                    .raised(palette)
                    .when(leading, |surface| {
                        surface.border_r_1().border_color(rgba(palette.menu_border))
                    })
                    .when(!leading, |surface| {
                        surface.border_l_1().border_color(rgba(palette.menu_border))
                    })
                    .elevated(Elevation::High)
                    .opacity(1.0 - shown);
                panel =
                    panel
                        .child(surface)
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
                        .child(self.sidebar_body(region, palette, cx)),
                ),
            );
        }
        if self.sidebar.visible && !self.sidebar.over_page() {
            layer = layer.child(self.resize_handle(region, cx));
        }
        let navigation = gpui::div()
            .absolute()
            .top(px(top_extent))
            .when(leading, |navigation| navigation.left_0())
            .when(!leading, |navigation| navigation.right_0())
            .child(navigation);
        layer.child(navigation).into_any_element()
    }

    /// Renders the visible side panels in the order stored by the workspace.
    fn sidebar_body(
        &self,
        region: DockRegion,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let layout = WorkspaceLayout::current(cx);
        let mut sections = Vec::new();
        for component in layout.components(region) {
            match component.id {
                ComponentId::AddressField => {
                    sections.push(SidebarSection::Address(
                        self.omnibox.clone().into_any_element(),
                    ));
                }
                ComponentId::FavouritesPanel => {
                    let settings = Settings::get(cx).sidebar.clone();
                    let favourites = self.favourite_tiles(&settings.favourites, palette, cx);
                    if !favourites.is_empty() {
                        sections.push(SidebarSection::Favourites(
                            favourites_grid(favourites, palette).into_any_element(),
                        ));
                    }
                }
                ComponentId::VerticalTabs => {
                    sections.push(SidebarSection::Tabs(self.sidebar_tabs(palette, cx)));
                }
                ComponentId::SidebarFooter => {
                    sections.push(SidebarSection::Footer(self.sidebar_footer(palette, cx)));
                }
                _ => {}
            }
        }
        sidebar(sections, palette)
    }

    pub(super) fn sidebar_tabs(&self, palette: ThemeColors, cx: &mut Context<Self>) -> AnyElement {
        let space = self.current_space(cx);
        let tabs = tab_list(
            {
                let listed = self.listed_tabs(cx);
                self.tab_items(cx)
                    .into_iter()
                    .filter(|item| listed.contains(&item.index))
                    .collect()
            },
            Box::new(cx.listener(|this, _, window, cx| {
                cx.stop_propagation();
                this.open_command_bar(window, cx);
            })),
            Box::new(cx.listener(|this, dragged: &DraggedTab, _, cx| {
                this.drop_tab_into_today(dragged.index, cx);
            })),
            palette,
        );
        match self
            .spaces
            .arriving_from(Entrance::slide_in(Edge::Left).total_duration())
        {
            Some(edge) => gpui::div()
                .w_full()
                .child(tabs)
                .animate_in(("space-tabs", space as usize), Entrance::slide_in(edge))
                .into_any_element(),
            None => tabs.into_any_element(),
        }
    }

    pub(super) fn sidebar_footer(
        &self,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        footer(
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
        .into_any_element()
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
}

#[derive(Clone, Copy, Default)]
struct NavigationState {
    can_go_back: bool,
    can_go_forward: bool,
    can_reload: bool,
    loading: bool,
}

/// Anchors the toolbar menu below the menu button's bottom-right corner.
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
