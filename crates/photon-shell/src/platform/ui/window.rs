//! GPUI window bootstrap and top-level browser layout.

use gpui::{
    Anchor, AnyElement, App, ClickEvent, Context, Entity, EntityId, FocusHandle, Global,
    KeyDownEvent, MouseButton, MouseDownEvent, Point, QuitMode, Render, StyleRefinement,
    Subscription, WeakEntity, Window, WindowAppearance, anchored, div, point, prelude::*, px,
};
use gpui_elements::editable_text::actions::{DEFAULT_INPUT_CONTEXT, default_bindings};
use gpui_platform::application;
use photon_core::BrowserCommand;
use photon_shortcuts::{
    CloseTab, FocusOmnibox, GoBack, GoForward, NewTab, NewWindow, Reload, ReopenClosedTab,
    SelectLastTab, SelectNextTab, SelectPreviousTab, SelectTab1, SelectTab2, SelectTab3,
    SelectTab4, SelectTab5, SelectTab6, SelectTab7, SelectTab8, StopLoading, browser_shortcuts,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use super::super::engine::{EngineRuntime, UiWake};
use super::super::motion_observer::ReducedMotionObserver;
use super::super::trace;
use super::super::window_observer::WindowObserver;
use super::super::window_settings;
use super::crash_alert::crash_alert;
use super::icons::LOADING_SPINNER_STEPS;
use super::layout::v_stack;
use super::menu::{
    menu_action, menu_checkbox, menu_radio, menu_section, menu_separator, menu_surface,
};
use super::omnibox::Omnibox;
use super::tabs::{ICON_APPEAR_DURATION, RevealedIcon, TabIcon, TabItem, tab_strip};
use super::titlebar::titlebar;
use super::toolbar::{ClickHandler, address_toolbar as build_address_toolbar};
use super::{PhotonWebView, WebViewEvent};
use super::{
    metrics,
    theme::{ThemeColors, ThemePreference},
};

struct BrowserWindow {
    tabs: Vec<Entity<PhotonWebView>>,
    /// One per tab, in `tabs` order: re-renders the chrome on page state changes.
    tab_subscriptions: Vec<Subscription>,
    /// Titlebar and toolbar, cached so Engine frames repaint only the page.
    chrome: Entity<BrowserChrome>,
    tab_focus_handles: Vec<FocusHandle>,
    active_tab: usize,
    omnibox: Entity<Omnibox>,
    runtime: Rc<EngineRuntime>,
    theme: ThemePreference,
    open_menu: Option<OpenMenu>,
    /// Pages of closed tabs, most recent last, for reopening.
    closed_tabs: Vec<ClosedTab>,
    /// The loading spinners' animation step, shared by every loading tab.
    spinner_step: usize,
    /// Whether a timer is advancing `spinner_step`.
    spinner_running: bool,
    /// The icon each tab last revealed and when, so an icon animates in once
    /// rather than whenever the tab redraws it after a spinner.
    revealed_icons: RefCell<HashMap<EntityId, (RevealedIcon, Instant)>>,
    /// Whether any part of the window is on screen. Engine renders the active
    /// tab only while it is, since GPUI-CE stops drawing an occluded window.
    window_visible: bool,
    _appearance_subscription: Subscription,
    _chrome_subscription: Subscription,
    _window_observer: Option<WindowObserver>,
}

/// The titlebar and address toolbar. A cached view re-renders only when it is
/// notified, so `BrowserWindow` notifies it whenever the window or a tab's page
/// state changes; Engine frames, which notify only the page view, leave it be.
struct BrowserChrome {
    browser: WeakEntity<BrowserWindow>,
}

impl Render for BrowserChrome {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.browser
            .update(cx, |browser, cx| browser.render_chrome(window, cx))
            .unwrap_or_else(|_| div().into_any_element())
    }
}

struct ClosedTab {
    index: usize,
    url: String,
}

/// How many closed tabs a window remembers.
const CLOSED_TAB_LIMIT: usize = 25;

/// Time between loading spinner steps.
const SPINNER_STEP_INTERVAL: Duration = Duration::from_millis(80);

/// Keeps the system reduced-motion preference flowing into GPUI.
struct ReducedMotion {
    _observer: Option<ReducedMotionObserver>,
}

impl Global for ReducedMotion {}

#[derive(Clone, Copy)]
enum OpenMenu {
    Context(Point<gpui::Pixels>),
    Toolbar(Point<gpui::Pixels>),
}

impl BrowserWindow {
    fn active_webview(&self) -> Entity<PhotonWebView> {
        self.tabs[self.active_tab].clone()
    }

    fn subscribe_to_tab(webview: &Entity<PhotonWebView>, cx: &mut Context<Self>) -> Subscription {
        cx.subscribe(webview, |this, _, _: &WebViewEvent, cx| {
            this.chrome.update(cx, |_, cx| cx.notify());
            this.animate_spinner(cx);
        })
    }

    fn any_tab_loading(&self, cx: &App) -> bool {
        self.tabs.iter().any(|tab| {
            let view = tab.read(cx);
            view.state.loading && view.has_page()
        })
    }

    /// Steps the loading spinners while any tab loads. With reduced motion
    /// they stay still.
    fn animate_spinner(&mut self, cx: &mut Context<Self>) {
        if self.spinner_running || cx.reduce_motion() || !self.any_tab_loading(cx) {
            return;
        }
        self.spinner_running = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(SPINNER_STEP_INTERVAL).await;
                let keep_running = this
                    .update(cx, |this, cx| {
                        let keep_running = !cx.reduce_motion() && this.any_tab_loading(cx);
                        if keep_running {
                            this.spinner_step = (this.spinner_step + 1) % LOADING_SPINNER_STEPS;
                            if this.window_visible {
                                this.chrome.update(cx, |_, cx| cx.notify());
                            }
                        } else {
                            this.spinner_running = false;
                        }
                        keep_running
                    })
                    .unwrap_or(false);
                if !keep_running {
                    break;
                }
            }
        })
        .detach();
    }

    fn render_chrome(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        v_stack()
            .w_full()
            .child(titlebar(self.tab_strip(window, cx)))
            .child(self.address_toolbar(window, cx))
            .into_any_element()
    }

    fn window_changed(&mut self, visible: bool, display_changed: bool, cx: &mut Context<Self>) {
        trace(format_args!(
            "window visible={visible} display-changed={display_changed}"
        ));
        if self.window_visible != visible {
            self.window_visible = visible;
            self.active_webview()
                .update(cx, |view, _| view.session.set_visible(visible));
        }
        if display_changed {
            for tab in self.tabs.clone() {
                tab.update(cx, |view, cx| {
                    view.session.forget_display();
                    cx.notify();
                });
            }
        }
    }

    fn set_theme(&mut self, appearance: WindowAppearance, cx: &mut Context<Self>) {
        self.theme.set(appearance);
        cx.set_window_appearance(Some(appearance));

        for tab in self.tabs.clone() {
            let _ = tab.update(cx, |_, cx| cx.notify());
        }
        let _ = self.omnibox.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    fn activate_tab(
        &mut self,
        index: usize,
        focus_contents: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.tabs.len() {
            return;
        }
        if index != self.active_tab {
            if let Some(previous) = self.tabs.get(self.active_tab) {
                previous.update(cx, |view, _| {
                    view.session.set_visible(false);
                    view.session.set_focus(false);
                });
            }
            self.active_tab = index;
        }
        let webview = self.tabs[index].clone();
        let is_blank_tab = webview.read(cx).is_blank_tab();
        let window_visible = self.window_visible;
        webview.update(cx, |view, cx| {
            view.session.set_visible(window_visible);
            if focus_contents && !is_blank_tab {
                window.focus(&view.focus_handle, cx);
            }
            view.track_engine_focus(window, cx);
        });
        self.omnibox.update(cx, |omnibox, cx| {
            omnibox.set_webview(webview, window, cx);
            if focus_contents && is_blank_tab {
                omnibox.focus(window, cx);
            }
        });
        cx.notify();
    }

    fn move_tab_focus(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.activate_tab(index, false, window, cx);
        window.focus(&self.tab_focus_handles[index], cx);
    }

    fn open_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.insert_tab(self.tabs.len(), None, window, cx);
    }

    fn insert_tab(
        &mut self,
        index: usize,
        address: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let webview = create_webview(
            cx,
            self.runtime.clone(),
            self.theme.clone(),
            address,
            address.is_none(),
        );
        let index = index.min(self.tabs.len());
        self.tab_subscriptions
            .insert(index, Self::subscribe_to_tab(&webview, cx));
        self.tabs.insert(index, webview);
        self.tab_focus_handles
            .insert(index, cx.focus_handle().tab_stop(false));
        if index <= self.active_tab {
            // Keep `active_tab` naming the same page until the switch below.
            self.active_tab += 1;
        }
        self.activate_tab(index, true, window, cx);
    }

    fn reopen_closed_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(closed) = self.closed_tabs.pop() {
            self.insert_tab(closed.index, Some(&closed.url), window, cx);
        }
    }

    /// An action handler that activates the tab at `index`, if there is one.
    fn select_tab_at<A>(index: usize) -> impl Fn(&mut Self, &A, &mut Window, &mut Context<Self>) {
        move |this, _, window, cx| this.activate_tab(index, true, window, cx)
    }

    /// Activates the tab `offset` places from the active one, wrapping at the ends.
    fn select_relative_tab(&mut self, offset: isize, window: &mut Window, cx: &mut Context<Self>) {
        let count = self.tabs.len() as isize;
        let index = (self.active_tab as isize + offset).rem_euclid(count) as usize;
        self.activate_tab(index, true, window, cx);
    }

    fn close_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }

        let removed = self.tabs[index].clone();
        let favicon = removed.update(cx, |view, _| {
            view.session.set_visible(false);
            view.session.set_focus(false);
            view.favicon.take()
        });
        if let Some(favicon) = favicon {
            cx.drop_image(favicon.image, Some(window));
        }
        self.revealed_icons
            .borrow_mut()
            .remove(&removed.entity_id());

        if self.tabs.len() == 1 {
            window.remove_window();
            return;
        }

        let view = removed.read(cx);
        if view.has_page() {
            let url = view.state.url.clone();
            if self.closed_tabs.len() == CLOSED_TAB_LIMIT {
                self.closed_tabs.remove(0);
            }
            self.closed_tabs.push(ClosedTab { index, url });
        }

        let was_active = index == self.active_tab;
        self.tabs.remove(index);
        drop(self.tab_subscriptions.remove(index));
        self.tab_focus_handles.remove(index);
        if index < self.active_tab {
            self.active_tab -= 1;
        } else if was_active {
            self.active_tab = index.min(self.tabs.len() - 1);
        }

        if was_active {
            self.activate_tab(self.active_tab, true, window, cx);
        } else {
            cx.notify();
        }
    }

    fn dispatch_command(
        &mut self,
        command: BrowserCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_menu = None;
        match command {
            BrowserCommand::NewTab => self.open_tab(window, cx),
            BrowserCommand::NewWindow => {
                if let Err(error) = open_browser_window(
                    self.runtime.clone(),
                    initial_address(),
                    self.theme.clone(),
                    cx,
                ) {
                    super::super::trace(format_args!("new window: {error:#}"));
                }
            }
            command => {
                let dismiss_crash_alert = matches!(&command, BrowserCommand::Reload);
                let webview = self.active_webview();
                if let Err(error) = webview.update(cx, |view, cx| {
                    if dismiss_crash_alert {
                        view.crash_alert = false;
                    }
                    let result = view.session.execute(command);
                    cx.notify();
                    result
                }) {
                    super::super::trace(format_args!("browser command: {error:#}"));
                }
            }
        }
        cx.notify();
    }

    fn toggle_performance_overlay(&mut self, cx: &mut Context<Self>) {
        self.open_menu = None;
        let webview = self.active_webview();
        webview.update(cx, |view, cx| {
            view.set_performance_overlay_enabled(!view.performance_overlay_enabled);
            cx.notify();
        });
        cx.notify();
    }

    fn tab_strip(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = ThemeColors::for_appearance(self.theme.appearance(window.appearance()));
        let tabs = self
            .tabs
            .iter()
            .enumerate()
            .map(|(index, webview)| {
                let view = webview.read(cx);
                let state = view.state.clone();
                let is_blank = !view.has_page();
                let icon = if is_blank {
                    TabIcon::NewTab
                } else if view.shows_spinner() {
                    TabIcon::Loading(self.spinner_step)
                } else if let Some(favicon) = view.favicon.clone() {
                    TabIcon::Favicon(favicon)
                } else {
                    TabIcon::Page
                };
                let icon_appearing = self.icon_appearing(webview.entity_id(), &icon);
                let tab_id = format!("browser-tab-{:?}", webview.entity_id());
                let label = if is_blank {
                    "New Tab".into()
                } else if !state.title.trim().is_empty() {
                    state.title
                } else {
                    state.url
                };
                TabItem {
                    id: tab_id,
                    label,
                    icon,
                    icon_appearing,
                    active: index == self.active_tab,
                    focus_handle: self.tab_focus_handles[index].clone(),
                    on_select: Box::new(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.activate_tab(index, true, window, cx);
                    })),
                    on_key_down: Box::new(cx.listener(
                        move |this, event: &KeyDownEvent, window, cx| {
                            if event.keystroke.modifiers.modified() {
                                return;
                            }
                            let next_index = match event.keystroke.key.as_str() {
                                "left" => Some((index + this.tabs.len() - 1) % this.tabs.len()),
                                "right" => Some((index + 1) % this.tabs.len()),
                                _ => None,
                            };
                            if let Some(next_index) = next_index {
                                this.move_tab_focus(next_index, window, cx);
                                window.prevent_default();
                                cx.stop_propagation();
                            }
                        },
                    )),
                    on_close: Box::new(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.close_tab(index, window, cx);
                    })),
                }
            })
            .collect();

        tab_strip(
            tabs,
            Box::new(cx.listener(|this, _, window, cx| {
                cx.stop_propagation();
                this.open_tab(window, cx);
            })),
            palette,
        )
    }

    /// Whether a tab's icon is still within its appear animation. A newly
    /// revealed icon starts it; the same icon shown again does not.
    fn icon_appearing(&self, tab: EntityId, icon: &TabIcon) -> bool {
        let Some(revealed) = icon.revealed() else {
            return false;
        };
        let mut revealed_icons = self.revealed_icons.borrow_mut();
        match revealed_icons.get(&tab) {
            Some((icon, since)) if *icon == revealed => since.elapsed() < ICON_APPEAR_DURATION,
            _ => {
                revealed_icons.insert(tab, (revealed, Instant::now()));
                true
            }
        }
    }

    fn address_toolbar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = ThemeColors::for_appearance(self.theme.appearance(window.appearance()));
        let view = self.active_webview().read(cx);
        let state = view.state.clone();
        let can_reload = view.has_page();
        let loading = state.loading;
        let menu_open = matches!(self.open_menu, Some(OpenMenu::Toolbar(_)));

        build_address_toolbar(
            self.omnibox.clone(),
            state.can_go_back,
            state.can_go_forward,
            can_reload,
            loading,
            menu_open,
            palette,
            Box::new(cx.listener(|this, _, window, cx| {
                this.dispatch_command(BrowserCommand::Back, window, cx);
            })),
            Box::new(cx.listener(|this, _, window, cx| {
                this.dispatch_command(BrowserCommand::Forward, window, cx);
            })),
            Box::new(cx.listener(move |this, _, window, cx| {
                let command = if loading {
                    BrowserCommand::StopLoading
                } else {
                    BrowserCommand::Reload
                };
                this.dispatch_command(command, window, cx);
            })),
            Box::new(cx.listener(|this, event: &ClickEvent, _, cx| {
                cx.stop_propagation();
                this.open_menu = match this.open_menu {
                    Some(OpenMenu::Toolbar(_)) => None,
                    _ => Some(OpenMenu::Toolbar(toolbar_menu_anchor(event))),
                };
                cx.notify();
            })),
        )
    }

    fn browser_menu(
        &self,
        appearance: WindowAppearance,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + 'static {
        let performance_overlay_enabled =
            self.active_webview().read(cx).performance_overlay_enabled;
        let is_dark = matches!(
            appearance,
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        );
        let light_appearance = WindowAppearance::Light;
        let dark_appearance = WindowAppearance::Dark;

        let content = v_stack()
            .gap(px(metrics::MENU_ITEM_GAP))
            .child(menu_action(
                "menu-new-tab",
                "New Tab",
                0,
                Box::new(cx.listener(|this, _, window, cx| {
                    cx.stop_propagation();
                    this.dispatch_command(BrowserCommand::NewTab, window, cx);
                })),
                palette,
            ))
            .child(menu_action(
                "menu-new-window",
                "New Window",
                1,
                Box::new(cx.listener(|this, _, window, cx| {
                    cx.stop_propagation();
                    this.dispatch_command(BrowserCommand::NewWindow, window, cx);
                })),
                palette,
            ))
            .child(menu_checkbox(
                "menu-debug-info",
                "Debug info",
                2,
                performance_overlay_enabled,
                Box::new(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.toggle_performance_overlay(cx);
                })),
                palette,
            ))
            .child(menu_separator(palette))
            .child(menu_section("Theme", palette))
            .child(menu_radio(
                "menu-theme-light",
                "Light",
                3,
                !is_dark,
                Box::new(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.open_menu = None;
                    this.set_theme(light_appearance, cx);
                })),
                palette,
            ))
            .child(menu_radio(
                "menu-theme-dark",
                "Dark",
                4,
                is_dark,
                Box::new(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.open_menu = None;
                    this.set_theme(dark_appearance, cx);
                })),
                palette,
            ));

        menu_surface(content, palette)
    }
}

impl Render for BrowserWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let appearance = self.theme.appearance(window.appearance());
        let palette = ThemeColors::for_appearance(appearance);
        let mut root = v_stack()
            .size_full()
            .relative()
            .bg(gpui::rgba(palette.window_tint))
            .text_color(gpui::rgb(palette.text_primary))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    if f32::from(event.position.y) <= metrics::TITLEBAR_HEIGHT {
                        this.open_menu = Some(OpenMenu::Context(event.position));
                        cx.stop_propagation();
                        cx.notify();
                    }
                }),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    if this.open_menu.is_some() {
                        this.open_menu = None;
                        window.prevent_default();
                        cx.stop_propagation();
                        cx.notify();
                    } else if let Some(tab) = this
                        .tabs
                        .iter()
                        .find(|tab| tab.read(cx).crash_alert)
                        .cloned()
                    {
                        tab.update(cx, |view, cx| {
                            view.crash_alert = false;
                            cx.notify();
                        });
                        window.prevent_default();
                        cx.stop_propagation();
                        cx.notify();
                    }
                }
            }))
            .on_action(cx.listener(|this, _: &FocusOmnibox, window, cx| {
                this.omnibox
                    .update(cx, |omnibox, cx| omnibox.focus(window, cx));
            }))
            .on_action(cx.listener(|this, _: &NewTab, window, cx| {
                this.dispatch_command(BrowserCommand::NewTab, window, cx);
            }))
            .on_action(cx.listener(|this, _: &NewWindow, window, cx| {
                this.dispatch_command(BrowserCommand::NewWindow, window, cx);
            }))
            .on_action(cx.listener(|this, _: &CloseTab, window, cx| {
                this.open_menu = None;
                this.close_tab(this.active_tab, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ReopenClosedTab, window, cx| {
                this.open_menu = None;
                this.reopen_closed_tab(window, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectNextTab, window, cx| {
                this.select_relative_tab(1, window, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectPreviousTab, window, cx| {
                this.select_relative_tab(-1, window, cx);
            }))
            .on_action(cx.listener(Self::select_tab_at::<SelectTab1>(0)))
            .on_action(cx.listener(Self::select_tab_at::<SelectTab2>(1)))
            .on_action(cx.listener(Self::select_tab_at::<SelectTab3>(2)))
            .on_action(cx.listener(Self::select_tab_at::<SelectTab4>(3)))
            .on_action(cx.listener(Self::select_tab_at::<SelectTab5>(4)))
            .on_action(cx.listener(Self::select_tab_at::<SelectTab6>(5)))
            .on_action(cx.listener(Self::select_tab_at::<SelectTab7>(6)))
            .on_action(cx.listener(Self::select_tab_at::<SelectTab8>(7)))
            .on_action(cx.listener(|this, _: &SelectLastTab, window, cx| {
                this.activate_tab(this.tabs.len() - 1, true, window, cx);
            }))
            .on_action(cx.listener(|this, _: &Reload, window, cx| {
                this.dispatch_command(BrowserCommand::Reload, window, cx);
            }))
            .on_action(cx.listener(|this, _: &StopLoading, window, cx| {
                this.dispatch_command(BrowserCommand::StopLoading, window, cx);
            }))
            .on_action(cx.listener(|this, _: &GoBack, window, cx| {
                this.dispatch_command(BrowserCommand::Back, window, cx);
            }))
            .on_action(cx.listener(|this, _: &GoForward, window, cx| {
                this.dispatch_command(BrowserCommand::Forward, window, cx);
            }))
            .child(
                self.chrome.clone().cached(
                    StyleRefinement::default()
                        .w_full()
                        .h(px(metrics::CHROME_HEIGHT)),
                ),
            )
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .p(px(metrics::PAGE_INSET))
                    .child(self.active_webview()),
            );
        if let Some(tab) = self
            .tabs
            .iter()
            .find(|tab| tab.read(cx).crash_alert)
            .cloned()
        {
            let reload_tab = tab.clone();
            let dismiss_tab = tab;
            let on_reload: ClickHandler = Box::new(cx.listener(move |_, _, _, cx| {
                reload_tab.update(cx, |view, cx| {
                    view.crash_alert = false;
                    if let Err(error) = view.session.execute(BrowserCommand::Reload) {
                        eprintln!("Photon Engine: retrying crashed page failed: {error:#}");
                    }
                    cx.notify();
                });
                cx.notify();
            }));
            let on_dismiss: ClickHandler = Box::new(cx.listener(move |_, _, _, cx| {
                dismiss_tab.update(cx, |view, cx| {
                    view.crash_alert = false;
                    cx.notify();
                });
                cx.notify();
            }));
            root = root.child(crash_alert(palette, on_reload, on_dismiss));
        }
        if let Some(open_menu) = self.open_menu {
            let appearance = self.theme.appearance(window.appearance());
            let palette = ThemeColors::for_appearance(appearance);
            let backdrop = div()
                .absolute()
                .inset_0()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.open_menu = None;
                        cx.stop_propagation();
                        cx.notify();
                    }),
                )
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(|this, _, _, cx| {
                        this.open_menu = None;
                        cx.stop_propagation();
                        cx.notify();
                    }),
                );
            let menu = self.browser_menu(appearance, palette, cx);
            let (anchor, position) = match open_menu {
                OpenMenu::Context(position) => (Anchor::TopLeft, position),
                OpenMenu::Toolbar(position) => (Anchor::TopRight, position),
            };
            root = root.child(backdrop).child(
                anchored()
                    .anchor(anchor)
                    .position(position)
                    .snap_to_window()
                    .child(menu),
            );
        }
        root
    }
}

fn toolbar_menu_anchor(event: &ClickEvent) -> Point<gpui::Pixels> {
    match event {
        ClickEvent::Keyboard(event) => event.bounds.bottom_right(),
        ClickEvent::Mouse(_) | ClickEvent::Touch(_) => {
            let position = event.position();
            let button_center_offset = px(metrics::TOOLBAR_BUTTON_SIZE / 2.0);
            point(
                position.x + button_center_offset,
                position.y + button_center_offset,
            )
        }
    }
}

pub fn run() {
    application().run(|cx: &mut App| {
        // GPUI keeps macOS apps alive after their last window closes by default. Photon
        // has one browser window and owns an Engine runtime, so closing it must quit the
        // app and run the registered Engine/GPU shutdown path.
        cx.set_quit_mode(QuitMode::LastWindowClosed);
        cx.bind_keys(default_bindings().as_keybindings(Some(DEFAULT_INPUT_CONTEXT)));
        // Keep browser actions and their default bindings in the shortcuts crate.
        cx.bind_keys(browser_shortcuts());
        let app = cx.to_async();
        let observer = ReducedMotionObserver::new(move |reduce_motion| {
            app.spawn(async move |cx| cx.update(|cx| cx.set_reduce_motion(reduce_motion)))
                .detach();
        });
        cx.set_global(ReducedMotion {
            _observer: observer,
        });
        let runtime = Rc::new(
            EngineRuntime::create()
                .unwrap_or_else(|error| panic!("could not start Photon Engine: {error:#}")),
        );
        quit_after_env_timeout(cx);
        cx.activate(true);
        open_browser_window(runtime, initial_address(), ThemePreference::default(), cx)
            .expect("open GPUI-CE Photon window");
    });
}

fn initial_address() -> Option<String> {
    std::env::var("PHOTON_URL")
        .ok()
        .map(|address| address.trim().to_owned())
        .filter(|address| !address.is_empty())
}

fn open_browser_window(
    runtime: Rc<EngineRuntime>,
    startup_address: Option<String>,
    theme: ThemePreference,
    cx: &mut App,
) -> anyhow::Result<()> {
    let is_blank_tab = startup_address.is_none();
    let webview = create_webview(
        cx,
        runtime.clone(),
        theme.clone(),
        startup_address.as_deref(),
        is_blank_tab,
    );
    cx.open_window(window_settings::options(cx), move |window, cx| {
        window.set_window_title("Photon");
        webview.update(cx, |view, cx| {
            view.session.set_visible(true);
            if !is_blank_tab {
                window.focus(&view.focus_handle, cx);
            }
            view.track_engine_focus(window, cx);
        });
        let omnibox = cx.new(|cx| Omnibox::new(webview.clone(), window, cx));
        if is_blank_tab {
            omnibox.update(cx, |omnibox, cx| omnibox.focus(window, cx));
        }
        cx.new(move |cx| {
            let appearance_subscription =
                cx.observe_window_appearance(window, |_, _, cx| cx.notify());
            let browser = cx.entity().downgrade();
            let chrome = cx.new(|_| BrowserChrome { browser });
            // Whatever re-renders the window's own state re-renders the chrome too.
            let chrome_subscription =
                cx.observe_self(|this, cx| this.chrome.update(cx, |_, cx| cx.notify()));
            let tab_subscriptions = vec![BrowserWindow::subscribe_to_tab(&webview, cx)];
            let this = cx.entity().downgrade();
            let app = cx.to_async();
            let window_observer = WindowObserver::new(window, move |visible, display_changed| {
                let this = this.clone();
                app.spawn(async move |cx| {
                    this.update(cx, |this: &mut BrowserWindow, cx| {
                        this.window_changed(visible, display_changed, cx)
                    })
                    .ok();
                })
                .detach();
            });
            BrowserWindow {
                tabs: vec![webview],
                tab_subscriptions,
                chrome,
                tab_focus_handles: vec![cx.focus_handle().tab_stop(true)],
                active_tab: 0,
                omnibox,
                runtime,
                theme,
                open_menu: None,
                closed_tabs: Vec::new(),
                spinner_step: 0,
                spinner_running: false,
                revealed_icons: RefCell::default(),
                window_visible: true,
                _appearance_subscription: appearance_subscription,
                _chrome_subscription: chrome_subscription,
                _window_observer: window_observer,
            }
        })
    })?;
    Ok(())
}

/// Starts the Engine-backed WebView, wired to redraw on Engine frames and to drain
/// GPU work before the app quits.
fn create_webview(
    cx: &mut App,
    runtime: Rc<EngineRuntime>,
    theme: ThemePreference,
    startup_address: Option<&str>,
    is_blank_tab: bool,
) -> Entity<PhotonWebView> {
    let startup_address = startup_address.map(str::to_owned);
    let webview = cx.new(|cx| {
        let mut webview =
            PhotonWebView::new(cx, runtime, theme, startup_address.as_deref(), is_blank_tab)
                .unwrap_or_else(|error| panic!("could not start direct PhotonWebView: {error:#}"));
        let gpu_activity = webview.session.gpu_activity.clone();
        webview._quit_subscription = Some(cx.on_app_quit(
            move |view: &mut PhotonWebView, _cx: &mut Context<'_, PhotonWebView>| {
                view.prepare_shutdown();
                gpu_activity.wait_until_idle();
                view.session.finish_shutdown();
                async {}
            },
        ));
        webview
    });
    let wake = UiWake {
        app: cx.to_async(),
        webview: webview.downgrade(),
    };
    webview.update(cx, |view, _| view.session.set_ui_wake(wake));
    webview
}

/// Quits after PHOTON_SHUTDOWN_AFTER_SECONDS, for timed shutdown checks.
fn quit_after_env_timeout(cx: &mut App) {
    let Some(seconds) = std::env::var("PHOTON_SHUTDOWN_AFTER_SECONDS")
        .ok()
        .and_then(|seconds| seconds.parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
    else {
        return;
    };
    let timer = cx.background_executor().timer(Duration::from_secs(seconds));
    cx.spawn(async move |cx| {
        timer.await;
        cx.update(|cx| cx.quit());
    })
    .detach();
}
