//! Builds browser chrome from catalog components. Toolbar controls and side
//! panels are placed independently by the active workspace layout.

use gpui::{AnyElement, Context, Div, Window, div, prelude::*, px};
use photon_core::{Command, DockRegion};

use super::super::button::{menu_button, toolbar_button};
use super::super::icons::{back_icon, close_icon, forward_icon, reload_icon};
use super::super::layout::{h_stack, v_stack};
use super::super::settings::Settings;
use super::super::tabs::{TabDragAxis, TabDragMove, tab_strip};
use super::super::titlebar::titlebar;
use super::super::toolbar::toolbar_row;
use super::super::workspace_layout::{ComponentId, WorkspaceLayout};
use super::super::{ClickHandler, metrics};
use super::BrowserWindow;

impl BrowserWindow {
    /// Renders catalog components into the four docks around the page. Zen
    /// mode slides the bars above and below off the window's edges.
    pub(super) fn render_chrome(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let layout = WorkspaceLayout::current(cx);
        let tucked = 1.0 - self.sidebar.chrome_shown();
        let mut root = div().size_full().relative();
        if tucked < 1.0 {
            let top_titlebar = (!layout.has_sidebar()
                || layout
                    .components(DockRegion::Top)
                    .any(|component| component.id != ComponentId::TabStrip))
            .then(|| self.blank_titlebar(window, cx));
            if let Some(top) = self.dock_bar(DockRegion::Top, top_titlebar, window, cx) {
                root = root.child(
                    top.absolute()
                        .top(px(-layout.top_extent() * tucked))
                        .left_0()
                        .right_0(),
                );
            }
            if let Some(bottom) = self.dock_bar(DockRegion::Bottom, None, window, cx) {
                root = root.child(
                    bottom
                        .absolute()
                        .bottom(px(-layout.bottom_extent() * tucked))
                        .left_0()
                        .right_0(),
                );
            }
        }
        if let Some(region) = layout.sidebar_region() {
            root = root.child(self.render_sidebar(region, window, cx));
        }
        root.into_any_element()
    }

    /// A bar above or below the page: its tab strip, or else `titlebar`,
    /// then a toolbar row with the other controls placed there.
    fn dock_bar(
        &self,
        region: DockRegion,
        titlebar: Option<Div>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let layout = WorkspaceLayout::current(cx);
        let has_tab_strip = layout
            .components(region)
            .any(|component| component.id == ComponentId::TabStrip);
        let controls = layout
            .components(region)
            .filter(|component| component.id != ComponentId::TabStrip)
            .map(|component| self.render_component(component.id, component.region, window, cx))
            .collect::<Vec<_>>();
        let heading = if has_tab_strip {
            Some(self.render_component(ComponentId::TabStrip, region, window, cx))
        } else {
            titlebar.map(IntoElement::into_any_element)
        };
        if heading.is_none() && controls.is_empty() {
            return None;
        }
        Some(
            v_stack()
                .w_full()
                .children(heading)
                .when(!controls.is_empty(), |bar| bar.child(toolbar_row(controls))),
        )
    }

    /// Empty titlebar space that moves the window, above the sidebar when
    /// the tabs are there, or across the window.
    fn blank_titlebar(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let layout = WorkspaceLayout::current(cx);
        let area = div().h(px(metrics::TITLEBAR_HEIGHT)).flex_shrink_0();
        let content = if let Some(region) = layout
            .sidebar_region()
            .filter(|_| layout.has_vertical_tabs())
        {
            let width = self.sidebar.docked_width(cx);
            h_stack()
                .w_full()
                .h(px(metrics::TITLEBAR_HEIGHT))
                .when(region == DockRegion::Trailing, |bar| bar.justify_end())
                .child(self.with_browser_menu(area.w(px(width)), cx))
        } else {
            self.with_browser_menu(area.w_full(), cx)
        };
        titlebar(content, window, self.palette(window, cx))
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
            let browser = cx.entity().downgrade();
            let on_drag_move: TabDragMove =
                std::rc::Rc::new(move |event, phase, hitbox, window, cx| {
                    browser
                        .update(cx, |this, cx| {
                            this.update_tab_drag(event, phase, hitbox, window, cx);
                        })
                        .ok();
                });
            let strip = tab_strip(
                self.tab_items(cx),
                self.new_tab_handler(cx),
                on_drag_move,
                self.tab_drop_handler(cx),
                self.tab_container_bounds_handler(cx),
                self.tab_strip_scroll.clone(),
                palette,
                crate::platform::supports_elastic_scroll() && Settings::get(cx).elastic_scroll,
                self.tab_reorder_groups
                    .group(TabDragAxis::Horizontal, true)
                    .clone(),
                self.tab_reorder_groups
                    .group(TabDragAxis::Horizontal, false)
                    .clone(),
            );
            return if region == DockRegion::Top {
                self.with_browser_menu(titlebar(strip, window, palette), cx)
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
                    this.execute_command(Command::GoBack, window, cx);
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
                    this.execute_command(Command::GoForward, window, cx);
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
                            Command::StopLoading
                        } else {
                            Command::ReloadPage
                        };
                        this.execute_command(command, window, cx);
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
                palette,
                Box::new(cx.listener(|this, _, window, cx| {
                    cx.stop_propagation();
                    this.show_browser_menu(window, cx);
                })),
            )
            .into_any_element(),
            ComponentId::SidebarToggle => self.sidebar_toggle(size, palette, cx).into_any_element(),
            ComponentId::TabStrip
            | ComponentId::FavouritesPanel
            | ComponentId::VerticalTabs
            | ComponentId::SidebarFooter => div().into_any_element(),
        }
    }

    fn new_tab_handler(&self, cx: &mut Context<Self>) -> ClickHandler {
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
