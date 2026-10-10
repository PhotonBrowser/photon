//! The browser window: its tabs, chrome, menus and page area.

mod actions;
mod alerts;
mod app;
mod content;
mod find;
mod menu;
mod page_menu;
mod popup_window;
mod popups;
mod tab_menu;
mod tabs;
mod zoom;

use gpui::{
    AnyElement, App, ClickEvent, Context, Entity, FocusHandle, MouseButton, MouseDownEvent, Render,
    StyleRefinement, Subscription, WeakEntity, Window, div, prelude::*, px,
};
use photon_core::BrowserCommand;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::time::Instant;

use super::super::engine::EngineRuntime;
use super::super::trace;
use super::super::window_observer::WindowObserver;
use super::super::window_settings;
use super::find_bar::{FIND_BAR_MOTION, FindBar};
use super::js_dialog::JavaScriptDialog;
use super::layout::v_stack;
use super::modal::MODAL_MOTION;
use super::motion::{AnimateIn, Presence};
use super::omnibox::{Omnibox, OmniboxEvent};
use super::pages::NEW_TAB;
use super::settings::Settings;
use super::tabs::RevealedIcon;
use super::titlebar::titlebar;
use super::toolbar::address_toolbar;
use super::{
    metrics,
    theme::{ThemeColors, palette},
};
use alerts::{EngineNotice, NoticeExpiry};
use content::{BrowserTab, TabContent, create_webview};
use menu::{OpenMenu, toolbar_menu_anchor};
use popups::PendingPopup;
use tabs::ClosedTab;

pub use app::run;

struct BrowserWindow {
    tabs: Vec<BrowserTab>,
    /// One per tab, in `tabs` order: follows its web view or page.
    tab_subscriptions: Vec<Subscription>,
    /// Titlebar and toolbar, cached so Engine frames repaint only the page.
    chrome: Entity<BrowserChrome>,
    tab_focus_handles: Vec<FocusHandle>,
    active_tab: usize,
    next_tab_id: u64,
    omnibox: Entity<Omnibox>,
    _omnibox_subscription: Subscription,
    runtime: Rc<EngineRuntime>,
    open_menu: Option<OpenMenu>,
    /// Keeps a closed menu drawn while it animates away.
    menu_presence: Presence<OpenMenu>,
    find_bar_presence: Presence<Entity<FindBar>>,
    dialog_presence: Presence<Entity<JavaScriptDialog>>,
    notice_presence: Presence<alerts::Notice>,
    /// Pages of closed tabs, most recent last, for reopening.
    closed_tabs: Vec<ClosedTab>,
    /// The loading spinners' animation step, shared by every loading tab.
    spinner_step: usize,
    /// Whether a timer is advancing `spinner_step`.
    spinner_running: bool,
    /// The open find bar, for the active tab.
    find_bar: Option<(Entity<FindBar>, Subscription)>,
    /// The active tab's open JavaScript dialog.
    dialog: Option<Entity<JavaScriptDialog>>,
    pending_popups: VecDeque<PendingPopup>,
    popup_confirmation_focus: FocusHandle,
    /// The last Engine service stop or restart, shown in a chip.
    engine_notice: Option<EngineNotice>,
    /// When the zoom chip was last shown, after a zoom change.
    zoom_shown_at: Option<Instant>,
    /// Redraws when a recovery chip expires.
    notice_expiry: NoticeExpiry,
    /// The icon each tab last revealed and when, so an icon animates in once
    /// rather than whenever the tab redraws it after a spinner.
    revealed_icons: RefCell<HashMap<u64, (RevealedIcon, Instant)>>,
    /// Whether any part of the window is on screen. Engine renders the active
    /// tab only while it is, since GPUI-CE stops drawing an occluded window.
    window_visible: bool,
    _appearance_subscription: Subscription,
    _settings_subscription: Subscription,
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
    fn palette(&self, window: &Window, cx: &App) -> ThemeColors {
        palette(window, cx)
    }

    fn render_chrome(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        v_stack()
            .w_full()
            .child(titlebar(self.tab_strip(window, cx)))
            .child(self.address_toolbar(window, cx))
            .into_any_element()
    }

    fn address_toolbar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (can_go_back, can_go_forward, can_reload, loading) =
            self.active_webview()
                .map_or((false, false, false, false), |webview| {
                    let view = webview.read(cx);
                    (
                        view.state.can_go_back,
                        view.state.can_go_forward,
                        view.has_page(),
                        view.state.loading,
                    )
                });

        address_toolbar(
            self.omnibox.clone(),
            can_go_back,
            can_go_forward,
            can_reload,
            loading,
            matches!(self.open_menu, Some(OpenMenu::Toolbar(_))),
            self.palette(window, cx),
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

    /// Redraws the chrome and every tab for changed settings, such as the theme.
    fn settings_changed(&mut self, cx: &mut Context<Self>) {
        for webview in self.tabs.iter().filter_map(|tab| tab.content.webview()) {
            webview.update(cx, |_, cx| cx.notify());
        }
        self.omnibox.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    fn window_changed(&mut self, visible: bool, display_changed: bool, cx: &mut Context<Self>) {
        trace(format_args!(
            "window visible={visible} display-changed={display_changed}"
        ));
        if self.window_visible != visible {
            self.window_visible = visible;
            if let Some(webview) = self.active_webview() {
                webview.update(cx, |view, _| view.session.set_visible(visible));
            }
        }
        if display_changed {
            for tab in self.tabs.iter().filter_map(|tab| tab.content.webview()) {
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
                if let Err(error) =
                    open_browser_window(self.runtime.clone(), app::initial_address(), cx)
                {
                    trace(format_args!("new window: {error:#}"));
                }
            }
            BrowserCommand::Navigate(url) => self.navigate_active(&url, window, cx),
            command => {
                if let Some(webview) = self.active_webview() {
                    if let Err(error) = webview.update(cx, |view, cx| {
                        let result = view.execute(command, cx);
                        cx.notify();
                        result
                    }) {
                        trace(format_args!("browser command: {error:#}"));
                    }
                }
            }
        }
        cx.notify();
    }
}

impl Render for BrowserWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette(window, cx);
        let find_bar = self.find_bar_presence.sync(
            self.find_bar.as_ref().map(|(bar, _)| bar.clone()),
            FIND_BAR_MOTION,
            cx,
        );
        // The dialog animates itself; this keeps a closed one drawn while it does.
        let dialog = self
            .dialog_presence
            .sync(self.dialog.clone(), MODAL_MOTION, cx);
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
                    // Keep tall pages inside the page area; they scroll themselves.
                    .min_h_0()
                    .overflow_hidden()
                    .w_full()
                    .p(px(metrics::PAGE_INSET))
                    .child(self.active_view(palette))
                    // The find bar floats in the page's top-right corner.
                    .children(find_bar.map(|(bar, transition)| {
                        div()
                            .absolute()
                            .top(px(metrics::PAGE_INSET + metrics::CHIP_INSET))
                            .right(px(metrics::PAGE_INSET + metrics::CHIP_INSET))
                            .child(div().child(bar).animate(
                                "find-bar-motion",
                                FIND_BAR_MOTION,
                                transition,
                            ))
                    }))
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
            .children(dialog.map(|(dialog, _)| dialog))
            .children(self.open_menu_overlay(palette, cx))
            .children(self.popup_confirmation(palette, cx));
        self.on_actions(root, cx)
    }
}

fn open_browser_window(
    runtime: Rc<EngineRuntime>,
    startup_address: Option<String>,
    cx: &mut App,
) -> anyhow::Result<()> {
    let initial_webview = startup_address
        .as_deref()
        .map(|address| create_webview(cx, runtime.clone(), address));
    cx.open_window(window_settings::options(cx), move |window, cx| {
        window.set_window_title(photon_brand::NAME);
        let omnibox = cx.new(|cx| Omnibox::new(window, cx));
        cx.new(move |cx: &mut Context<BrowserWindow>| {
            let appearance_subscription =
                cx.observe_window_appearance(window, |_, _, cx| cx.notify());
            // Follow settings changed here, in a settings page, or in another window.
            let settings_subscription =
                cx.observe_global::<Settings>(|this: &mut BrowserWindow, cx| {
                    this.settings_changed(cx)
                });
            let browser = cx.entity().downgrade();
            let chrome = cx.new(|_| BrowserChrome { browser });
            // Whatever re-renders the window's own state re-renders the chrome too.
            let chrome_subscription =
                cx.observe_self(|this, cx| this.chrome.update(cx, |_, cx| cx.notify()));
            let omnibox_subscription = cx.subscribe_in(
                &omnibox,
                window,
                |this, _, event: &OmniboxEvent, window, cx| match event {
                    OmniboxEvent::Navigate(url) => this.navigate_active(url, window, cx),
                },
            );
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
            let mut browser = BrowserWindow {
                tabs: Vec::new(),
                tab_subscriptions: Vec::new(),
                chrome,
                tab_focus_handles: Vec::new(),
                active_tab: 0,
                next_tab_id: 1,
                omnibox,
                _omnibox_subscription: omnibox_subscription,
                runtime,
                open_menu: None,
                menu_presence: Presence::default(),
                find_bar_presence: Presence::default(),
                dialog_presence: Presence::default(),
                notice_presence: Presence::default(),
                closed_tabs: Vec::new(),
                spinner_step: 0,
                spinner_running: false,
                find_bar: None,
                dialog: None,
                pending_popups: VecDeque::new(),
                popup_confirmation_focus: cx.focus_handle(),
                engine_notice: None,
                notice_expiry: None,
                zoom_shown_at: None,
                revealed_icons: RefCell::default(),
                window_visible: true,
                _appearance_subscription: appearance_subscription,
                _settings_subscription: settings_subscription,
                _chrome_subscription: chrome_subscription,
                _window_observer: window_observer,
            };
            match initial_webview {
                Some(webview) => {
                    let content = TabContent::Web(webview);
                    browser.insert_content(0, content, true, window, cx);
                }
                None => browser.insert_page(0, NEW_TAB, window, cx),
            }
            browser
        })
    })?;
    Ok(())
}
