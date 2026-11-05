//! Photon's own pages, drawn by the shell rather than loaded by the Engine,
//! at addresses such as `photon://settings`.
//!
//! Each page is a view with its own state. It reads and changes the shared
//! [`Settings`](super::settings::Settings) and history directly, and asks its
//! window for anything beyond itself through its [`PageContext`].
//!
//! To add a page, write a module here with its view and a
//! [`PageDefinition`], and list it in the owning shell plugin module. Build it from the shared
//! [`layout`] and [`controls`].

#[cfg(target_os = "macos")]
mod appkit;
mod controls;
mod downloads;
mod history;
mod layout;
mod new_tab;
mod performance;
mod registry;
mod settings;

use gpui::{
    AnyElement, AnyView, App, AppContext, Entity, EventEmitter, StyleRefinement, prelude::*, px,
    rgba,
};
use photon_core::{BrowserSettings, internal_page_url};
use std::rc::Rc;

use super::super::engine::EngineRuntime;
use super::settings::Settings;
use super::{ClickHandler, metrics, theme::ThemeColors};
#[cfg(target_os = "macos")]
pub(in crate::platform::ui) use registry::APPKIT;
pub(super) use registry::{
    DOWNLOADS, HISTORY, NEW_TAB, PERFORMANCE, PageDefinition, PageIcon, SETTINGS, find_page,
};

/// What a page asks its window to do.
pub(super) enum PageEvent {
    /// Load an address from the page.
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
    /// Whether the page is in a private window.
    pub(super) private: bool,
}

impl PageContext {
    /// Asks the window to load `url` in the active tab, creating one if needed.
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

/// One of Photon's pages, usually shown in a tab.
#[derive(Clone)]
pub(super) struct InternalPage {
    pub(super) definition: &'static PageDefinition,
    view: Entity<PageFrame>,
    events: Entity<PageEvents>,
}

impl InternalPage {
    pub(super) fn open(
        definition: &'static PageDefinition,
        runtime: &Rc<EngineRuntime>,
        private: bool,
        cx: &mut App,
    ) -> Self {
        let events = cx.new(|_| PageEvents);
        let context = PageContext {
            events: events.clone(),
            runtime: runtime.clone(),
            private,
        };
        Self {
            definition,
            view: {
                let frame_context = context.clone();
                cx.new(|cx| PageFrame::new(definition, frame_context, cx))
            },
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
    pub(super) fn view(&self, _palette: ThemeColors) -> AnyElement {
        self.view
            .clone()
            .cached(StyleRefinement::default().size_full())
            .into_any_element()
    }
}

/// Frames shell pages like the page viewport, including its material and corners.
pub(super) fn page_frame(content: AnyElement, palette: ThemeColors, radius: f32) -> AnyElement {
    gpui::div()
        .size_full()
        .overflow_hidden()
        .rounded(px(radius))
        .rounded_smoothing(metrics::page_smoothing())
        .bg(rgba(palette.page_surface))
        .backdrop_blur(px(palette.surface_blur))
        .child(content)
        .into_any_element()
}

/// Separates the live surface from an internal page's expensive form/layout.
struct PageFrame {
    definition: &'static PageDefinition,
    context: Option<PageContext>,
    body: Option<AnyView>,
    foreground: Option<u32>,
    _appearance: Option<gpui::Subscription>,
    _preview: gpui::Subscription,
    _settings: gpui::Subscription,
}
impl PageFrame {
    fn new(
        definition: &'static PageDefinition,
        context: PageContext,
        cx: &mut gpui::Context<Self>,
    ) -> Self {
        let body =
            Self::feature_enabled(definition, cx).then(|| (definition.build)(context.clone(), cx));
        Self::from_parts(definition, Some(context), body, cx)
    }

    fn from_parts(
        definition: &'static PageDefinition,
        context: Option<PageContext>,
        body: Option<AnyView>,
        cx: &mut gpui::Context<Self>,
    ) -> Self {
        Self {
            body,
            definition,
            context,
            foreground: None,
            _appearance: None,
            _preview: cx.observe_global::<super::appearance_preview::AppearancePreview>(|_, cx| {
                cx.notify()
            }),
            _settings: cx.observe_global::<Settings>(|this, cx| {
                if !Self::feature_enabled(this.definition, cx) {
                    this.body = None;
                }
                cx.notify();
            }),
        }
    }

    #[cfg(test)]
    fn new_test(body: AnyView, cx: &mut gpui::Context<Self>) -> Self {
        Self::from_parts(SETTINGS, None, Some(body), cx)
    }

    fn feature_enabled(definition: &PageDefinition, cx: &gpui::App) -> bool {
        super::window::plugins::page_enabled(definition, &Settings::get(cx).features)
    }
}
impl gpui::Render for PageFrame {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        if self._appearance.is_none() {
            self._appearance = Some(cx.observe_window_appearance(window, |_, _, cx| cx.notify()));
        }
        let palette = super::theme::page_palette(window, cx);
        if self
            .foreground
            .replace(palette.text_primary)
            .is_some_and(|ink| ink != palette.text_primary)
            && let Some(body) = &self.body
        {
            gpui::App::notify(cx, body.entity_id());
        }
        if !Self::feature_enabled(self.definition, cx) {
            self.body = None;
            let palette = super::theme::page_palette(window, cx);
            let message = format!(
                "{} is turned off. Re-enable it in Settings under Advanced.",
                self.definition.title
            );
            return page_frame(
                super::layout::v_stack()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .child(super::layout::secondary_text(message, palette).text_center())
                    .into_any_element(),
                palette,
                metrics::page_radius(),
            );
        }
        if self.body.is_none() {
            let context = self
                .context
                .as_ref()
                .expect("feature page retains its build context");
            self.body = Some((self.definition.build)(context.clone(), cx));
        }
        page_frame(
            self.body
                .as_ref()
                .expect("enabled page has a view")
                .clone()
                .cached(StyleRefinement::default().size_full())
                .into_any_element(),
            palette,
            metrics::page_radius(),
        )
    }
}

/// A click handler that changes the settings.
fn change(update: impl Fn(&mut BrowserSettings) + 'static) -> ClickHandler {
    Box::new(move |_, _, cx| Settings::update(cx, &update))
}

#[cfg(test)]
mod preview_tests;
