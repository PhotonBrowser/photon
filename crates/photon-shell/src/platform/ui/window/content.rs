//! What a tab shows — a web page in the Engine or one of Photon's own pages —
//! and how the window follows each.

use gpui::{AnyElement, App, Context, Entity, Subscription, Window, prelude::*};
use photon_core::BrowserCommand;
use std::rc::Rc;

use super::super::super::engine::{EngineRuntime, EngineSession, UiWake};
use super::super::super::trace;
use super::super::pages::{InternalPage, PageDefinition, PageEvent, find_page};
use super::super::settings::Settings;
use super::super::theme::ThemeColors;
use super::super::{PhotonWebView, WebViewEvent};
use super::BrowserWindow;

/// What a tab shows.
#[derive(Clone)]
pub(super) enum TabContent {
    Page(InternalPage),
    Web(Entity<PhotonWebView>),
}

impl TabContent {
    pub(super) fn webview(&self) -> Option<Entity<PhotonWebView>> {
        match self {
            Self::Page(_) => None,
            Self::Web(webview) => Some(webview.clone()),
        }
    }

    fn view(&self, palette: ThemeColors) -> AnyElement {
        match self {
            Self::Page(page) => page.view(palette),
            Self::Web(webview) => webview.clone().into_any_element(),
        }
    }

    /// The address the omnibox shows for one of Photon's pages, if any.
    pub(super) fn page_address(&self) -> Option<String> {
        match self {
            Self::Page(page) => page.address(),
            Self::Web(_) => None,
        }
    }
}

/// A tab: an identity that survives reordering, and what it shows.
#[derive(Clone)]
pub(super) struct BrowserTab {
    pub(super) id: u64,
    pub(super) content: TabContent,
}

impl BrowserWindow {
    pub(super) fn active_webview(&self) -> Option<Entity<PhotonWebView>> {
        self.tabs[self.active_tab].content.webview()
    }

    /// The active tab's page or web view.
    pub(super) fn active_view(&self, palette: ThemeColors) -> AnyElement {
        self.tabs[self.active_tab].content.view(palette)
    }

    pub(super) fn new_page(&self, page: &'static PageDefinition, cx: &mut App) -> TabContent {
        TabContent::Page(InternalPage::open(page, &self.runtime, cx))
    }

    /// Loads `url` in the active tab: one of Photon's pages for a
    /// `photon://` address, otherwise a web page, replacing whichever the tab
    /// showed when the kind changes.
    pub(super) fn navigate_active(
        &mut self,
        url: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(page) = find_page(url) {
            let content = self.new_page(page, cx);
            self.replace_active_content(content, window, cx);
            return;
        }
        if let Some(webview) = self.active_webview() {
            if let Err(error) = webview.update(cx, |view, cx| {
                view.execute(BrowserCommand::Navigate(url.to_owned()), cx)
            }) {
                trace(format_args!("navigation: {error:#}"));
            }
            cx.notify();
            return;
        }
        let webview = create_webview(cx, self.runtime.clone(), url);
        self.replace_active_content(TabContent::Web(webview), window, cx);
    }

    fn replace_active_content(
        &mut self,
        content: TabContent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(webview) = self.active_webview() {
            webview.update(cx, |view, _| {
                view.session.set_visible(false);
                view.session.set_focus(false);
            });
        }
        self.tab_subscriptions[self.active_tab] = self.subscribe_to_content(&content, window, cx);
        self.tabs[self.active_tab].content = content;
        self.activate_tab(self.active_tab, true, window, cx);
    }

    /// Follows the tab's web view or page for as long as the subscription lives.
    pub(super) fn subscribe_to_content(
        &self,
        content: &TabContent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        match content {
            TabContent::Web(webview) => Self::subscribe_to_webview(webview, window, cx),
            TabContent::Page(page) => Self::subscribe_to_page(page, window, cx),
        }
    }

    fn subscribe_to_page(
        page: &InternalPage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.subscribe_in(
            page.events(),
            window,
            |this, _, event: &PageEvent, window, cx| match event {
                PageEvent::Open(url) => this.navigate_active(url, window, cx),
                PageEvent::OpenPage(page) => this.open_page_tab(page, window, cx),
            },
        )
    }

    fn subscribe_to_webview(
        webview: &Entity<PhotonWebView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.subscribe_in(
            webview,
            window,
            |this, webview, event: &WebViewEvent, window, cx| match event {
                WebViewEvent::StateChanged => {
                    webview.update(cx, |view, cx| view.record_history(cx));
                    this.chrome.update(cx, |_, cx| cx.notify());
                    this.animate_spinner(cx);
                    this.sync_dialog(window, cx);
                    cx.notify();
                }
                WebViewEvent::ContextMenuRequested
                    if this.active_webview().as_ref() == Some(webview) =>
                {
                    this.open_page_menu(cx);
                }
                WebViewEvent::ContextMenuRequested => {}
                WebViewEvent::OpenInNewTab { url, activate } => {
                    this.open_tab_beside(webview, url, *activate, window, cx);
                }
                WebViewEvent::NewWebViewRequested(id) => {
                    if let Some(request) = webview.update(cx, |view, _| view.take_new_web_view(*id))
                    {
                        this.handle_requested_web_view(webview, request, window, cx);
                    }
                }
            },
        )
    }
}

/// Starts an Engine-backed web view loading `address`.
pub(super) fn create_webview(
    cx: &mut App,
    runtime: Rc<EngineRuntime>,
    address: &str,
) -> Entity<PhotonWebView> {
    let session = EngineSession::create(runtime.clone(), 1200, 760, 1.0, Some(address))
        .unwrap_or_else(|error| panic!("could not start direct PhotonWebView: {error:#}"));
    create_webview_from_session(cx, runtime, session)
}

/// Wraps an Engine session in a web view, wired to redraw on Engine frames
/// and to drain GPU work before the app quits.
pub(super) fn create_webview_from_session(
    cx: &mut App,
    runtime: Rc<EngineRuntime>,
    session: EngineSession,
) -> Entity<PhotonWebView> {
    let webview = cx.new(|cx| {
        let mut webview = PhotonWebView::from_session(cx, session);
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
        runtime,
    };
    webview.update(cx, |view, _| view.session.set_ui_wake(wake));
    webview
}

/// Lets a web view that may load before it is first drawn follow the
/// appearance the settings choose.
pub(super) fn follow_appearance(webview: &Entity<PhotonWebView>, window: &Window, cx: &mut App) {
    let appearance = Settings::appearance(window.appearance(), cx);
    webview.update(cx, |view, _| view.update_color_scheme(appearance));
}
