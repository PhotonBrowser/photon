//! The browser window: its tabs, chrome, menus and page area.

mod actions;
mod alerts;
mod app;
mod menu;
mod tabs;

use gpui::{
    AnyElement, App, ClickEvent, Context, Entity, EntityId, FocusHandle, MouseButton,
    MouseDownEvent, Render, StyleRefinement, Subscription, WeakEntity, Window, div, prelude::*, px,
};
use photon_core::BrowserCommand;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use super::super::engine::{EngineRuntime, UiWake};
use super::super::trace;
use super::super::window_observer::WindowObserver;
use super::super::window_settings;
use super::js_dialog::JavaScriptDialog;
use super::layout::v_stack;
use super::omnibox::Omnibox;
use super::tabs::RevealedIcon;
use super::titlebar::titlebar;
use super::toolbar::address_toolbar;
use super::{PhotonWebView, WebViewEvent};
use super::{
    metrics,
    theme::{ThemeColors, ThemePreference},
};
use alerts::{EngineNotice, NoticeExpiry};
use menu::{OpenMenu, toolbar_menu_anchor};
use tabs::ClosedTab;

pub use app::run;

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
    /// The active tab's open JavaScript dialog.
    dialog: Option<Entity<JavaScriptDialog>>,
    /// The last Engine service stop or restart, shown in a chip.
    engine_notice: Option<EngineNotice>,
    /// Redraws when a recovery chip expires.
    notice_expiry: NoticeExpiry,
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

impl BrowserWindow {
    fn active_webview(&self) -> Entity<PhotonWebView> {
        self.tabs[self.active_tab].clone()
    }

    fn palette(&self, window: &Window) -> ThemeColors {
        ThemeColors::for_appearance(self.theme.appearance(window.appearance()))
    }

    fn subscribe_to_tab(
        webview: &Entity<PhotonWebView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.subscribe_in(webview, window, |this, _, _: &WebViewEvent, window, cx| {
            this.chrome.update(cx, |_, cx| cx.notify());
            this.animate_spinner(cx);
            this.sync_dialog(window, cx);
        })
    }

    fn render_chrome(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        v_stack()
            .w_full()
            .child(titlebar(self.tab_strip(window, cx)))
            .child(self.address_toolbar(window, cx))
            .into_any_element()
    }

    fn address_toolbar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = self.active_webview().read(cx);
        let state = &view.state;
        let loading = state.loading;

        address_toolbar(
            self.omnibox.clone(),
            state.can_go_back,
            state.can_go_forward,
            view.has_page(),
            loading,
            matches!(self.open_menu, Some(OpenMenu::Toolbar(_))),
            self.palette(window),
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
                    app::initial_address(),
                    self.theme.clone(),
                    cx,
                ) {
                    trace(format_args!("new window: {error:#}"));
                }
            }
            command => {
                let webview = self.active_webview();
                if let Err(error) = webview.update(cx, |view, cx| {
                    let result = view.execute(command, cx);
                    cx.notify();
                    result
                }) {
                    trace(format_args!("browser command: {error:#}"));
                }
            }
        }
        cx.notify();
    }
}

impl Render for BrowserWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette(window);
        let root = v_stack()
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
            .child(
                self.chrome.clone().cached(
                    StyleRefinement::default()
                        .w_full()
                        .h(px(metrics::CHROME_HEIGHT)),
                ),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .w_full()
                    .p(px(metrics::PAGE_INSET))
                    .child(self.active_webview())
                    // Crash and restart chips sit in the page's bottom-right corner.
                    .children(self.notice_chip(palette, cx).map(|chip| {
                        div()
                            .absolute()
                            .right(px(metrics::PAGE_INSET + metrics::CHIP_INSET))
                            .bottom(px(metrics::PAGE_INSET + metrics::CHIP_INSET))
                            .child(chip)
                    })),
            )
            // A JavaScript dialog is modal to the whole window.
            .children(self.dialog.clone())
            .children(self.open_menu_overlay(palette, cx));
        self.on_actions(root, cx)
    }
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
            let tab_subscriptions = vec![BrowserWindow::subscribe_to_tab(&webview, window, cx)];
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
                dialog: None,
                engine_notice: None,
                notice_expiry: None,
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
