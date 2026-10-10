//! Builds the window's chrome from its state: the horizontal tab strip and
//! address toolbar, or the sidebar (navigation, address field, favourites,
//! tabs and footer), shown beside the page or revealed over it.

use gpui::{
    AnyElement, ClickEvent, Context, ImageSource, KeyDownEvent, MouseDownEvent, MouseUpEvent,
    ObjectFit, Window, img, point, prelude::*, px, rgba,
};
use photon_core::{BrowserCommand, Shortcut, TabLayout, site_name};

use super::super::history::BrowsingHistory;
use super::super::icons::globe_icon;
use super::super::layout::{Elevated, Elevation, Raised};
use super::super::pages::{PageIcon, SETTINGS};
use super::super::settings::Settings;
use super::super::sidebar::{
    FavouriteTile, FooterActions, NavigationActions, NavigationState, SidebarSections,
    favourites_grid, footer, navigation_bar, sidebar,
};
use super::super::tabs::{DraggedTab, TabIcon, TabItem, tab_list, tab_strip};
use super::super::titlebar::titlebar;
use super::super::toolbar::address_toolbar;
use super::super::{metrics, theme::ThemeColors};
use super::BrowserWindow;
use super::content::TabContent;
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
            .child(titlebar(strip).on_mouse_down(
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
        let navigation = self.with_titlebar_menu(
            navigation_bar(
                self.navigation_state(cx),
                out,
                self.sidebar.visible,
                self.navigation_actions(cx),
                palette,
            ),
            cx,
        );
        let mut layer = gpui::div().size_full().relative();
        if out > 0.0 {
            let width = metrics::SIDEBAR_WIDTH;
            let mut panel = gpui::div()
                .id("sidebar-panel")
                .absolute()
                .top_0()
                .bottom_0()
                .left(px((out - 1.0) * width))
                .w(px(width))
                .occlude();
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
        layer
            .child(gpui::div().absolute().top_0().left_0().child(navigation))
            .into_any_element()
    }

    /// Everything in the sidebar below its navigation row.
    fn sidebar_body(&self, palette: ThemeColors, cx: &mut Context<Self>) -> impl IntoElement {
        let sidebar_settings = Settings::get(cx).sidebar.clone();
        let favourites = self.favourite_tiles(&sidebar_settings.favourites, palette, cx);
        sidebar(
            SidebarSections {
                address: self.omnibox.clone().into_any_element(),
                favourites: (!favourites.is_empty())
                    .then(|| favourites_grid(favourites, palette).into_any_element()),
                tabs: tab_list(self.tab_items(cx), self.new_tab_handler(cx), palette)
                    .into_any_element(),
                footer: footer(
                    matches!(self.open_menu, Some(OpenMenu::Sidebar(_))),
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

    fn favourite_tiles(
        &self,
        favourites: &[Shortcut],
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> Vec<FavouriteTile> {
        let active_site = self
            .active_webview()
            .map(|webview| site_name(&webview.read(cx).state.url));
        favourites
            .iter()
            .enumerate()
            .map(|(index, favourite)| {
                let icon = match BrowsingHistory::favicon(&favourite.url, cx) {
                    Some(favicon) => img(ImageSource::Render(favicon.image))
                        .size(px(metrics::SIDEBAR_FAVOURITE_ICON_SIZE))
                        .object_fit(ObjectFit::Contain)
                        .into_any_element(),
                    None => {
                        globe_icon(palette.text_secondary, metrics::SIDEBAR_FAVOURITE_ICON_SIZE)
                            .into_any_element()
                    }
                };
                let url = favourite.url.clone();
                FavouriteTile {
                    label: if favourite.title.trim().is_empty() {
                        site_name(&favourite.url)
                    } else {
                        favourite.title.clone()
                    },
                    icon,
                    active: active_site.as_deref() == Some(site_name(&favourite.url).as_str()),
                    on_open: Box::new(cx.listener(move |this, _, window, cx| {
                        this.open_favourite(&url, window, cx);
                    })),
                    on_context_menu: Box::new(cx.listener(
                        move |this, event: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            this.open_menu = Some(OpenMenu::Favourite(index, event.position));
                            cx.notify();
                        },
                    )),
                }
            })
            .collect()
    }

    /// Switches to a tab already showing the favourite's site, or opens it
    /// in a new tab.
    fn open_favourite(&mut self, url: &str, window: &mut Window, cx: &mut Context<Self>) {
        let site = site_name(url);
        let open = self.tabs.iter().position(|tab| {
            tab.content
                .webview()
                .is_some_and(|webview| site_name(&webview.read(cx).state.url) == site)
        });
        match open {
            Some(index) => self.activate_tab(index, true, window, cx),
            None => {
                let index = self.tabs.len();
                self.insert_tab(index, Some(url), window, cx);
            }
        }
    }

    fn tab_items(&self, cx: &mut Context<Self>) -> Vec<TabItem> {
        self.tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| {
                let (label, icon) = match &tab.content {
                    TabContent::Page(page) => {
                        let icon = match page.definition.icon {
                            PageIcon::Logo => TabIcon::Logo,
                            PageIcon::Symbol(symbol) => TabIcon::Symbol(symbol),
                        };
                        (page.definition.title.to_owned(), icon)
                    }
                    TabContent::Web(webview) => {
                        let view = webview.read(cx);
                        let icon = if view.audio_playing {
                            TabIcon::Audio {
                                favicon: view.favicon.clone(),
                                muted: view.audio_muted,
                            }
                        } else if view.shows_spinner() {
                            TabIcon::Loading(self.spinner_step)
                        } else if let Some(favicon) = view.favicon.clone() {
                            TabIcon::Favicon(favicon)
                        } else {
                            TabIcon::Page
                        };
                        let label = if !view.state.title.trim().is_empty() {
                            view.state.title.clone()
                        } else if view.state.url.is_empty() {
                            "New Tab".to_owned()
                        } else {
                            view.state.url.clone()
                        };
                        (label, icon)
                    }
                };
                TabItem {
                    id: format!("browser-tab-{}", tab.id),
                    label,
                    icon_appearing: self.icon_appearing(tab.id, &icon),
                    icon,
                    active: index == self.active_tab,
                    focus_handle: self.tab_focus_handles[index].clone(),
                    on_select: Box::new(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.activate_tab(index, true, window, cx);
                    })),
                    on_key_down: Box::new(cx.listener(
                        move |this, event: &KeyDownEvent, window, cx| {
                            this.tab_key_down(index, event, window, cx);
                        },
                    )),
                    on_close: Box::new(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.close_tab(index, window, cx);
                    })),
                    on_toggle_audio: Box::new(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        if let Some(webview) =
                            this.tabs.get(index).and_then(|tab| tab.content.webview())
                        {
                            webview.update(cx, |view, cx| view.toggle_audio_mute(cx));
                        }
                    })),
                    on_middle_click: Box::new(cx.listener(
                        move |this, _: &MouseUpEvent, window, cx| {
                            cx.stop_propagation();
                            this.close_tab(index, window, cx);
                        },
                    )),
                    on_context_menu: Box::new(cx.listener(
                        move |this, event: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            this.open_menu = Some(OpenMenu::Tab(index, event.position));
                            cx.notify();
                        },
                    )),
                    on_drop: Box::new(cx.listener(move |this, dragged: &DraggedTab, _, cx| {
                        this.move_tab(dragged.index, index, cx);
                    })),
                }
            })
            .collect()
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
