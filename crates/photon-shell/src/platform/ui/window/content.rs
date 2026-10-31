//! What a tab shows — a web page in the Engine or one of Photon's own pages —
//! and how the window follows each.

use gpui::{AnyElement, App, Context, Entity, Subscription, Window, div, prelude::*, px};
use photon_core::{BrowserCommand, BrowserState, Shortcut, SpaceId};
use std::rc::Rc;
use std::time::Instant;

use super::super::super::engine::{EngineRuntime, EngineSession, UiWake};
use super::super::super::trace;
use super::super::Favicon;
use super::super::pages::{InternalPage, NEW_TAB, PageDefinition, PageEvent, find_page};
use super::super::settings::Settings;
use super::super::{PhotonWebView, WebViewEvent};
use super::super::{
    button::{ButtonKind, ButtonSize, button},
    layout::{secondary_text, v_stack},
    metrics,
    theme::ThemeColors,
};
use super::BrowserWindow;

/// What a tab shows.
#[derive(Clone)]
pub(super) enum TabContent {
    Page(InternalPage),
    Web(Entity<PhotonWebView>),
    Sleeping(SleepingTab),
}

#[derive(Clone)]
pub(super) struct SleepingTab {
    pub(super) state: BrowserState,
    pub(super) favicon: Option<Favicon>,
}

impl TabContent {
    pub(super) fn webview(&self) -> Option<Entity<PhotonWebView>> {
        match self {
            Self::Page(_) => None,
            Self::Web(webview) => Some(webview.clone()),
            Self::Sleeping(_) => None,
        }
    }

    fn view(&self, palette: ThemeColors) -> AnyElement {
        match self {
            Self::Page(page) => page.view(palette),
            Self::Web(webview) => webview.clone().into_any_element(),
            Self::Sleeping(_) => div().into_any_element(),
        }
    }

    /// The address the omnibox shows for one of Photon's pages, if any.
    pub(super) fn page_address(&self) -> Option<String> {
        match self {
            Self::Page(page) => page.address(),
            Self::Web(_) | Self::Sleeping(_) => None,
        }
    }
}

/// A tab: an identity that survives reordering, and what it shows.
#[derive(Clone)]
pub(super) struct BrowserTab {
    pub(super) id: u64,
    pub(super) content: TabContent,
    /// Where a pinned tab was pinned; `None` for today's tabs.
    pub(super) pin: Option<Shortcut>,
    /// The address of the favourite the tab belongs to. In the sidebar it
    /// shows as that favourite's tile rather than in the tab list.
    pub(super) favourite: Option<String>,
    /// The space the tab belongs to.
    pub(super) space: SpaceId,
    /// When the tab was last in use, for closing tabs left unused.
    pub(super) last_active: Instant,
}

impl BrowserTab {
    pub(super) fn new(id: u64, content: TabContent, pin: Option<Shortcut>, space: SpaceId) -> Self {
        Self {
            id,
            content,
            pin,
            space,
            favourite: None,
            last_active: Instant::now(),
        }
    }
}

impl BrowserWindow {
    pub(super) fn active_webview(&self) -> Option<Entity<PhotonWebView>> {
        self.active_tab
            .and_then(|index| self.tabs.get(index))
            .and_then(|tab| tab.content.webview())
    }

    /// The active tab's page or web view, or the New Tab page when empty.
    pub(super) fn active_view(&self, palette: ThemeColors, cx: &mut Context<Self>) -> AnyElement {
        let Some(index) = self.active_tab else {
            return self
                .empty_page
                .as_ref()
                .map_or_else(|| div().into_any_element(), |page| page.view(palette));
        };
        let Some(tab) = self.tabs.get(index) else {
            return div().into_any_element();
        };
        match &tab.content {
            TabContent::Sleeping(_) => v_stack()
                .size_full()
                .items_center()
                .justify_center()
                .gap(px(metrics::INTERNAL_PAGE_SECTION_GAP))
                .child(secondary_text(
                    "This page was put to sleep to free memory. Reloading restores its URL; unsaved state and in-page history are lost.",
                    palette,
                ).max_w(px(metrics::INTERNAL_PAGE_MAX_WIDTH)).text_center())
                .child(button(
                    "sleeping-page-reload",
                    "Reload page",
                    ButtonKind::Primary,
                    true,
                    ButtonSize::Regular,
                    palette,
                    Box::new(cx.listener(move |this, _, window, cx| {
                        this.activate_tab(index, true, window, cx);
                    })),
                ))
                .into_any_element(),
            content => content.view(palette),
        }
    }

    pub(super) fn new_page(&self, page: &'static PageDefinition, cx: &mut App) -> TabContent {
        TabContent::Page(InternalPage::open(page, &self.runtime, cx))
    }

    pub(super) fn show_empty_new_tab_page(
        &mut self,
        focus_omnibox: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let content = self.new_page(NEW_TAB, cx);
        self._empty_page_subscription = self.subscribe_to_content(&content, window, cx);
        self.empty_page = Some(content);
        if focus_omnibox {
            self.omnibox
                .update(cx, |omnibox, cx| omnibox.focus(window, cx));
            let omnibox = self.omnibox.clone();
            cx.defer_in(window, move |_, window, cx| {
                omnibox.update(cx, |omnibox, cx| omnibox.focus(window, cx));
            });
        }
        cx.notify();
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
        if self.active_tab.is_none() {
            match find_page(url) {
                Some(page) => self.insert_page(self.new_tab_index(cx), page, window, cx),
                None => {
                    if let Some(webview) = create_webview(cx, self.runtime.clone(), url) {
                        self.insert_webview(
                            self.new_tab_index(cx),
                            webview,
                            None,
                            true,
                            window,
                            cx,
                        );
                    }
                }
            }
            return;
        }
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
        let Some(webview) = create_webview(cx, self.runtime.clone(), url) else {
            return;
        };
        self.replace_active_content(TabContent::Web(webview), window, cx);
    }

    fn replace_active_content(
        &mut self,
        content: TabContent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(active_tab) = self.active_tab else {
            self.insert_content(self.tabs.len(), content, true, window, cx);
            return;
        };
        self.finish_tab_page_transition(cx);
        if let Some(webview) = self.active_webview() {
            webview.update(cx, |view, _| {
                view.set_page_visible(false);
                view.session.set_focus(false);
            });
        }
        self.tab_subscriptions[active_tab] = self.subscribe_to_content(&content, window, cx);
        self.tabs[active_tab].content = content;
        self.activate_tab(active_tab, true, window, cx);
    }

    /// Follows the tab's web view or page for as long as the subscription lives.
    pub(super) fn subscribe_to_content(
        &self,
        content: &TabContent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Subscription> {
        match content {
            TabContent::Web(webview) => Some(Self::subscribe_to_webview(webview, window, cx)),
            TabContent::Page(page) => Some(Self::subscribe_to_page(page, window, cx)),
            TabContent::Sleeping(_) => None,
        }
    }

    /// Reloads a discarded hidden page when its tab is selected.
    pub(super) fn wake_tab(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(tab) = self.tabs.get(index) else {
            return false;
        };
        let TabContent::Sleeping(sleeping) = &tab.content else {
            return true;
        };
        let state = sleeping.state.clone();
        let favicon = sleeping.favicon.clone();
        let Some(webview) = create_webview(cx, self.runtime.clone(), &state.url) else {
            return false;
        };
        webview.update(cx, |view, _| {
            view.favicon = favicon;
            view.requested = Some(Shortcut {
                title: state.title.clone(),
                url: state.url.clone(),
            });
        });
        let content = TabContent::Web(webview);
        self.tab_subscriptions[index] = self.subscribe_to_content(&content, window, cx);
        self.tabs[index].content = content;
        self.revealed_icons
            .borrow_mut()
            .remove(&self.tabs[index].id);
        true
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
                    this.wake_tab_sleep_scheduler();
                    webview.update(cx, |view, cx| view.record_history(cx));
                    this.chrome.update(cx, |_, cx| cx.notify());
                    this.animate_spinner(cx);
                    this.sync_dialog(window, cx);
                    cx.notify();
                }
                WebViewEvent::ContextMenuRequested
                    if this.active_webview().as_ref() == Some(webview) =>
                {
                    this.open_page_menu(window, cx);
                }
                WebViewEvent::ContextMenuRequested => {}
                WebViewEvent::SelectDropdownRequested
                    if this.active_webview().as_ref() == Some(webview) =>
                {
                    let dropdown =
                        webview.update(cx, |view, _| view.take_select_dropdown_request());
                    if let Some(dropdown) = dropdown {
                        this.show_select_dropdown(webview.clone(), dropdown, window, cx);
                    }
                }
                WebViewEvent::SelectDropdownRequested => {
                    webview.update(cx, |view, _| view.session.select_dropdown_closed(None));
                }
                WebViewEvent::OpenInNewTab { url, activate } => {
                    this.open_tab_beside(webview, url, *activate, window, cx);
                }
                WebViewEvent::Peek(url) => this.open_peek(url, window, cx),
                WebViewEvent::FullscreenChanged(fullscreen) => {
                    this.page_fullscreen_changed(webview, *fullscreen, window, cx);
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
) -> Option<Entity<PhotonWebView>> {
    let session = match EngineSession::create(runtime.clone(), 1200, 760, 1.0, Some(address)) {
        Ok(session) => session,
        Err(error) => {
            trace(format_args!("could not open {address}: {error:#}"));
            return None;
        }
    };
    let webview = create_webview_from_session(cx, runtime, session);
    webview.update(cx, |view, _| {
        view.requested = Some(photon_core::Shortcut {
            title: String::new(),
            url: address.to_owned(),
        });
    });
    Some(webview)
}

/// Wraps an Engine session in a web view, wired to redraw on Engine frames
/// and to release its frames before the app quits.
pub(super) fn create_webview_from_session(
    cx: &mut App,
    runtime: Rc<EngineRuntime>,
    session: EngineSession,
) -> Entity<PhotonWebView> {
    let webview = cx.new(|cx| {
        let mut webview = PhotonWebView::from_session(cx, session);
        webview._quit_subscription = Some(cx.on_app_quit(
            move |view: &mut PhotonWebView, _cx: &mut Context<'_, PhotonWebView>| {
                view.prepare_shutdown();
                view.session.finish_shutdown();
                async {}
            },
        ));
        webview
    });
    // Apply profile preferences before the initial viewport starts navigation.
    let force_dark_mode = Settings::get(cx).force_dark_mode;
    webview.update(cx, |view, _| view.update_force_dark_mode(force_dark_mode));
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
    let force_dark_mode = Settings::get(cx).force_dark_mode;
    webview.update(cx, |view, _| {
        view.update_color_scheme(appearance);
        view.update_force_dark_mode(force_dark_mode);
    });
}
