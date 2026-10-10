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
mod sidebar;
mod tab_menu;
mod tabs;
mod zoom;

use gpui::{
    App, Context, Entity, FocusHandle, Render, StyleRefinement, Subscription, WeakEntity, Window,
    div, prelude::*, px,
};
use photon_core::{BrowserCommand, TabLayout};
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
use super::layout::{h_stack, v_stack};
use super::modal::MODAL_MOTION;
use super::motion::{AnimateIn, Presence};
use super::omnibox::{Omnibox, OmniboxEvent};
use super::pages::{NEW_TAB, find_page};
use super::settings::Settings;
use super::tabs::RevealedIcon;
use super::{
    metrics,
    theme::{ThemeColors, palette},
};
use alerts::{EngineNotice, NoticeExpiry};
use content::{BrowserTab, TabContent, create_webview};
use menu::OpenMenu;
use popups::PendingPopup;
use tabs::ClosedTab;

pub use app::run;

struct BrowserWindow {
    tabs: Vec<BrowserTab>,
    /// One per tab, in `tabs` order: follows its web view or page.
    tab_subscriptions: Vec<Subscription>,
    /// The sidebar, cached so Engine frames repaint only the page.
    chrome: Entity<BrowserChrome>,
    /// Whether the sidebar is shown; hidden, the page takes the whole width.
    sidebar_visible: bool,
    /// When the sidebar was last shown, so it slides in only then.
    sidebar_shown_at: Option<Instant>,
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

/// The tab strip and toolbar, or the sidebar. A cached view re-renders only when it is
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

    /// Whether the chrome is a sidebar beside the page, rather than bars above it.
    fn sidebar_beside_page(&self, cx: &App) -> bool {
        Settings::get(cx).tab_layout == TabLayout::Vertical && self.sidebar_visible
    }

    /// The height of the chrome above the page.
    fn chrome_height(&self, cx: &App) -> f32 {
        match Settings::get(cx).tab_layout {
            TabLayout::Horizontal => metrics::CHROME_HEIGHT,
            TabLayout::Vertical => metrics::TITLEBAR_HEIGHT,
        }
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
        let beside = self.sidebar_beside_page(cx);
        let page = div()
            .relative()
            .flex_1()
            // Keep tall pages inside the page area; they scroll themselves.
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .size_full()
            .p(px(metrics::PAGE_INSET))
            // Beside the sidebar, its own padding already separates the two.
            .when(beside, |page| page.pl_0())
            .child(self.active_view(palette))
            // The find bar floats in the page's top-right corner.
            .children(find_bar.map(|(bar, transition)| {
                div()
                    .absolute()
                    .top(px(metrics::PAGE_INSET + metrics::CHIP_INSET))
                    .right(px(metrics::PAGE_INSET + metrics::CHIP_INSET))
                    .child(
                        div()
                            .child(bar)
                            .animate("find-bar-motion", FIND_BAR_MOTION, transition),
                    )
            }))
            // Crash and restart chips sit in the page's bottom-right corner.
            .children(self.notice_chip(palette, cx).map(|chip| {
                div()
                    .absolute()
                    .right(px(metrics::PAGE_INSET + metrics::CHIP_INSET))
                    .bottom(px(metrics::PAGE_INSET + metrics::CHIP_INSET))
                    .child(chip)
            }));
        // The sidebar sits beside the page. The tab strip and toolbar sit
        // above it, as does the slim bar shown while the sidebar is hidden.
        let chrome_style = if beside {
            StyleRefinement::default()
                .w(px(metrics::SIDEBAR_WIDTH))
                .h_full()
                .flex_shrink_0()
        } else {
            StyleRefinement::default()
                .w_full()
                .h(px(self.chrome_height(cx)))
                .flex_shrink_0()
        };
        let chrome = self.chrome.clone().cached(chrome_style);
        let body = if beside {
            h_stack().size_full().child(chrome).child(page)
        } else {
            v_stack().size_full().child(chrome).child(page)
        };
        let root = div()
            .size_full()
            .relative()
            .bg(gpui::rgba(palette.window_tint))
            .text_color(gpui::rgb(palette.text_primary))
            .child(body)
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
    // A page's address opens that page; any other address a web page.
    let startup_page = startup_address.as_deref().and_then(find_page);
    let initial_webview = startup_address
        .as_deref()
        .filter(|_| startup_page.is_none())
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
                sidebar_visible: true,
                sidebar_shown_at: None,
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
                None => browser.insert_page(0, startup_page.unwrap_or(NEW_TAB), window, cx),
            }
            browser
        })
    })?;
    Ok(())
}
