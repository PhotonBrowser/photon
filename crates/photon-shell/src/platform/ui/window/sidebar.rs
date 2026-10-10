//! Builds the sidebar from the window's state: navigation, the address
//! field, favourites, the current space's tabs and the footer. Also the slim
//! bar shown while the sidebar is hidden, and showing or hiding it.

use gpui::{
    AnyElement, ClickEvent, Context, ImageSource, KeyDownEvent, MouseDownEvent, MouseUpEvent,
    ObjectFit, Window, img, point, prelude::*, px,
};
use photon_core::{BrowserCommand, Shortcut, site_name};
use std::time::Instant;

use super::super::history::BrowsingHistory;
use super::super::icons::globe_icon;
use super::super::motion::{AnimateIn, Edge, Entrance};
use super::super::pages::{PageIcon, SETTINGS};
use super::super::settings::Settings;
use super::super::sidebar::{
    DraggedTab, FavouriteTile, FooterActions, NavigationActions, NavigationState, SidebarSections,
    TabIcon, TabItem, favourites_grid, footer, navigation_bar, sidebar, space_header, tab_list,
};
use super::super::{metrics, theme::ThemeColors};
use super::BrowserWindow;
use super::content::TabContent;
use super::menu::OpenMenu;

/// How the sidebar slides in when shown.
const SIDEBAR_MOTION: Entrance = Entrance::slide_in(Edge::Left);

impl BrowserWindow {
    /// The sidebar, or the slim bar shown while it is hidden.
    pub(super) fn render_sidebar(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let palette = self.palette(window, cx);
        if !self.sidebar_visible {
            return navigation_bar(None, self.navigation_actions(cx), palette).into_any_element();
        }
        let sidebar_settings = Settings::get(cx).sidebar.clone();
        let space = sidebar_settings.active_space();
        let favourites = self.favourite_tiles(&sidebar_settings.favourites, palette, cx);
        let sections = SidebarSections {
            navigation: navigation_bar(
                Some(self.navigation_state(cx)),
                self.navigation_actions(cx),
                palette,
            )
            .on_mouse_down(
                gpui::MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.open_menu = Some(OpenMenu::Context(event.position));
                    cx.notify();
                }),
            )
            .into_any_element(),
            address: self.omnibox.clone().into_any_element(),
            favourites: (!favourites.is_empty())
                .then(|| favourites_grid(favourites, palette).into_any_element()),
            space: space_header(&space, palette).into_any_element(),
            tabs: tab_list(
                self.tab_items(cx),
                Box::new(cx.listener(|this, _, window, cx| {
                    cx.stop_propagation();
                    this.open_tab(window, cx);
                })),
                palette,
            )
            .into_any_element(),
            footer: footer(
                &space,
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
        };
        let sidebar = sidebar(sections, palette);
        // The sidebar slides in just after it is shown; drawn again later, as
        // when the window redraws, it appears at once.
        match self.sidebar_shown_at {
            Some(shown) if shown.elapsed() < SIDEBAR_MOTION.total_duration() => gpui::div()
                .size_full()
                .child(sidebar)
                .animate_in("sidebar-motion", SIDEBAR_MOTION)
                .into_any_element(),
            _ => sidebar.into_any_element(),
        }
    }

    /// Shows or hides the sidebar. Hidden, the page takes the whole width.
    pub(super) fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.set_sidebar_visible(!self.sidebar_visible, cx);
    }

    pub(super) fn set_sidebar_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        if self.sidebar_visible == visible {
            return;
        }
        self.sidebar_visible = visible;
        self.sidebar_shown_at = visible.then(Instant::now);
        self.open_menu = None;
        cx.notify();
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
                        .size(px(metrics::TAB_FAVICON_SIZE))
                        .object_fit(ObjectFit::Contain)
                        .into_any_element(),
                    None => globe_icon(palette.text_secondary, metrics::TAB_FAVICON_SIZE)
                        .into_any_element(),
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
