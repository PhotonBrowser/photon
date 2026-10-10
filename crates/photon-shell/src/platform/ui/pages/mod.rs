//! Photon's own pages, drawn by the shell in a tab rather than loaded by the
//! Engine, at addresses such as `photon://settings`.
//!
//! Each page is a view with its own state. It reads and changes the shared
//! [`Settings`](super::settings::Settings) and history directly, and asks its
//! window for anything beyond itself through its [`PageContext`].
//!
//! To add a page, write a module here with its view and a
//! [`PageDefinition`], and list it in [`registry`]. Build it from the shared
//! [`layout`] and [`controls`].

mod controls;
mod layout;
mod new_tab;
mod registry;
mod settings;

use gpui::{AnyElement, AnyView, App, AppContext, Entity, EventEmitter, prelude::*, px, rgba};
use photon_core::{BrowserSettings, internal_page_url};
use std::rc::Rc;

use super::super::engine::EngineRuntime;
use super::settings::Settings;
use super::{ClickHandler, metrics, theme::ThemeColors};
pub(super) use registry::{NEW_TAB, PageDefinition, PageIcon, SETTINGS, find_page};

/// What a page asks its window to do.
pub(super) enum PageEvent {
    /// Load an address in the page's tab.
    Open(String),
    /// Open another of Photon's pages.
    OpenPage(&'static PageDefinition),
}

/// Carries a page's requests to its window.
pub(super) struct PageEvents;

impl EventEmitter<PageEvent> for PageEvents {}

/// What a page is built with.
#[derive(Clone)]
pub(super) struct PageContext {
    events: Entity<PageEvents>,
    pub(super) runtime: Rc<EngineRuntime>,
}

impl PageContext {
    /// Asks the window to load `url` in the page's tab.
    pub(super) fn open(&self, url: String, cx: &mut App) {
        self.events
            .update(cx, |_, cx| cx.emit(PageEvent::Open(url)));
    }

    /// Asks the window to open another of Photon's pages.
    pub(super) fn open_page(&self, page: &'static PageDefinition, cx: &mut App) {
        self.events
            .update(cx, |_, cx| cx.emit(PageEvent::OpenPage(page)));
    }
}

/// One of Photon's pages, shown in a tab.
#[derive(Clone)]
pub(super) struct InternalPage {
    pub(super) definition: &'static PageDefinition,
    view: AnyView,
    events: Entity<PageEvents>,
}

impl InternalPage {
    pub(super) fn open(
        definition: &'static PageDefinition,
        runtime: &Rc<EngineRuntime>,
        cx: &mut App,
    ) -> Self {
        let events = cx.new(|_| PageEvents);
        let context = PageContext {
            events: events.clone(),
            runtime: runtime.clone(),
        };
        Self {
            definition,
            view: (definition.build)(context, cx),
            events,
        }
    }

    /// Where the page's requests arrive.
    pub(super) fn events(&self) -> &Entity<PageEvents> {
        &self.events
    }

    /// The address the omnibox shows, if the page shows one.
    pub(super) fn address(&self) -> Option<String> {
        self.definition
            .shows_address
            .then(|| internal_page_url(self.definition.name))
    }

    /// The page in a frame shaped like a web page's.
    pub(super) fn view(&self, palette: ThemeColors) -> AnyElement {
        gpui::div()
            .size_full()
            .overflow_hidden()
            .rounded(px(metrics::WEBVIEW_CORNER_RADIUS))
            .bg(rgba(palette.internal_page_surface))
            .child(self.view.clone())
            .into_any_element()
    }
}

/// A click handler that changes the settings.
fn change(update: impl Fn(&mut BrowserSettings) + 'static) -> ClickHandler {
    Box::new(move |_, _, cx| Settings::update(cx, &update))
}
