//! The reduced browser window used for site-created browsing contexts.

use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, Render, Subscription, Window, div, prelude::*,
    px, rgb,
};
use std::rc::Rc;

use crate::platform::engine::{EngineRuntime, RequestedWebView};

use super::super::js_dialog::DialogHost;
use super::super::layout::v_stack;
use super::super::modal::MODAL_MOTION;
use super::super::motion::Presence;
use super::super::settings::Settings;
use super::super::titlebar::titlebar;
use super::super::{
    PhotonWebView, WebViewEvent, metrics,
    theme::{ThemeColors, palette},
};
use super::content::{create_webview_from_session, follow_appearance};
use super::popups::popup_confirmation_dialog;
use super::window_settings;

struct PendingPopup {
    request: RequestedWebView,
    origin: String,
}

struct MinimalPopupWindow {
    webview: Entity<PhotonWebView>,
    runtime: Rc<EngineRuntime>,
    pending_popups: Vec<PendingPopup>,
    confirmation_focus: FocusHandle,
    /// Keeps the pop-up confirmation drawn while it fades away.
    popup_presence: Presence<String>,
    /// The page's JavaScript dialog, as a tab shows it.
    dialog: DialogHost,
    _webview_subscription: Subscription,
    _settings_subscription: Subscription,
}

pub(super) fn open_minimal_window(
    runtime: Rc<EngineRuntime>,
    request: RequestedWebView,
    cx: &mut App,
) {
    let options = window_settings::popup_options(cx, request.width, request.height);
    let animations = Settings::get(cx).animations;
    let webview = create_webview_from_session(cx, runtime.clone(), request.session);
    if let Err(error) = cx.open_window(options, move |window, cx| {
        window_settings::configure_native_controls(window, &animations);
        window.set_window_title(photon_brand::NAME);
        follow_appearance(&webview, window, cx);
        webview.update(cx, |view, cx| {
            view.session.set_visible(true);
            if request.activate {
                window.focus(&view.focus_handle, cx);
            }
            view.track_engine_focus(window, cx);
        });
        cx.new(|cx| {
            let webview_subscription = cx.subscribe_in(
                &webview,
                window,
                |this: &mut MinimalPopupWindow, webview, event: &WebViewEvent, window, cx| {
                    match event {
                        WebViewEvent::StateChanged => {
                            this.dialog.sync(Some(webview), window, cx);
                            cx.notify();
                        }
                        WebViewEvent::NewWebViewRequested(id) => {
                            let request = webview.update(cx, |view, _| view.take_new_web_view(*id));
                            if let Some(request) = request {
                                this.handle_requested_web_view(webview, request, window, cx);
                            }
                        }
                        // A site window has no menus or tabs of its own.
                        WebViewEvent::ContextMenuRequested
                        | WebViewEvent::OpenInNewTab { .. }
                        | WebViewEvent::Peek(_) => {}
                        #[cfg(target_os = "macos")]
                        WebViewEvent::SleepEligibilityChanged => {}
                    }
                },
            );
            MinimalPopupWindow {
                webview,
                runtime,
                pending_popups: Vec::new(),
                confirmation_focus: cx.focus_handle(),
                popup_presence: Presence::default(),
                dialog: DialogHost::default(),
                _webview_subscription: webview_subscription,
                // Follow theme and transparency changes made in any window.
                _settings_subscription: cx.observe_global_in::<Settings>(
                    window,
                    |this, window, cx| {
                        let animations = Settings::get(cx).animations;
                        window_settings::configure_native_controls(window, &animations);
                        this.webview.update(cx, |_, cx| cx.notify());
                        cx.notify();
                    },
                ),
            }
        })
    }) {
        eprintln!("Photon: could not open site window: {error:#}");
    }
}

impl MinimalPopupWindow {
    fn handle_requested_web_view(
        &mut self,
        source: &Entity<PhotonWebView>,
        request: RequestedWebView,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if request.popup && request.needs_confirmation {
            let origin = source.read(cx).state.url.clone();
            let should_focus = self.pending_popups.is_empty();
            self.pending_popups.push(PendingPopup { request, origin });
            if should_focus {
                window.focus(&self.confirmation_focus, cx);
            }
            cx.notify();
        } else {
            open_minimal_window(self.runtime.clone(), request, cx);
        }
    }

    fn popup_confirmation(
        &mut self,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let animations = Settings::get(cx).animations;
        let origin = self
            .pending_popups
            .first()
            .map(|popup| popup.origin.clone());
        let (origin, transition) =
            self.popup_presence
                .sync(origin, MODAL_MOTION, &animations, cx)?;
        Some(popup_confirmation_dialog(
            origin,
            transition,
            &self.confirmation_focus,
            palette,
            Self::block_pending_popup,
            Self::allow_pending_popup,
            cx,
        ))
    }

    fn allow_pending_popup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.pending_popups.is_empty() {
            let popup = self.pending_popups.remove(0);
            open_minimal_window(self.runtime.clone(), popup.request, cx);
        }
        self.refocus_after_popup_decision(window, cx);
    }

    fn block_pending_popup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.pending_popups.is_empty() {
            self.pending_popups.remove(0);
        }
        self.refocus_after_popup_decision(window, cx);
    }

    fn refocus_after_popup_decision(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending_popups.is_empty() {
            let focus = self.webview.read(cx).focus_handle.clone();
            window.focus(&focus, cx);
        } else {
            window.focus(&self.confirmation_focus, cx);
        }
        cx.notify();
    }
}

impl Render for MinimalPopupWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = palette(window, cx);
        let url = self.webview.read(cx).state.url.clone();
        v_stack()
            .size_full()
            .relative()
            .bg(palette.window_background())
            .text_color(rgb(palette.text_primary))
            .child(titlebar(
                div()
                    .w_full()
                    .text_size(px(metrics::POPUP_URL_FONT_SIZE))
                    .text_color(rgb(palette.text_secondary))
                    .truncate()
                    .child(url),
                window,
                palette,
            ))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .w_full()
                    .p(px(metrics::PAGE_INSET))
                    .child(self.webview.clone()),
            )
            .children(self.dialog.overlay(cx))
            .children(self.popup_confirmation(palette, cx))
    }
}
