//! The reduced browser window used for site-created browsing contexts.

use gpui::{
    App, Context, Entity, FocusHandle, KeyDownEvent, Render, Role, Subscription, Window, div,
    prelude::*, px, rgb, rgba,
};
use std::rc::Rc;

use crate::platform::engine::{EngineRuntime, RequestedWebView};

use super::super::layout::v_stack;
use super::super::modal::{modal, modal_panel};
use super::super::motion::Transition;
use super::super::titlebar::titlebar;
use super::super::{PhotonWebView, WebViewEvent, metrics, theme::ThemeColors};
use super::{create_webview_from_session, window_settings};

struct PendingPopup {
    request: RequestedWebView,
    origin: String,
}

struct MinimalPopupWindow {
    webview: Entity<PhotonWebView>,
    runtime: Rc<EngineRuntime>,
    theme: super::super::theme::ThemePreference,
    pending_popups: Vec<PendingPopup>,
    confirmation_focus: FocusHandle,
    _webview_subscription: Subscription,
}

pub(super) fn open_minimal_window(
    runtime: Rc<EngineRuntime>,
    theme: super::super::theme::ThemePreference,
    request: RequestedWebView,
    cx: &mut App,
) {
    let options = window_settings::popup_options(cx, request.width, request.height);
    let webview =
        create_webview_from_session(cx, runtime.clone(), theme.clone(), request.session, false);
    if let Err(error) = cx.open_window(options, move |window, cx| {
        window.set_window_title("Photon");
        webview.update(cx, |view, cx| {
            view.update_color_scheme(theme.appearance(window.appearance()));
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
                        WebViewEvent::StateChanged => cx.notify(),
                        WebViewEvent::NewWebViewRequested(id) => {
                            let request = webview.update(cx, |view, _| view.take_new_web_view(*id));
                            if let Some(request) = request {
                                this.handle_requested_web_view(webview, request, window, cx);
                            }
                        }
                        // A site window has no menus or tabs of its own.
                        WebViewEvent::ContextMenuRequested | WebViewEvent::OpenInNewTab { .. } => {}
                    }
                },
            );
            MinimalPopupWindow {
                webview,
                runtime,
                theme,
                pending_popups: Vec::new(),
                confirmation_focus: cx.focus_handle(),
                _webview_subscription: webview_subscription,
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
            open_minimal_window(self.runtime.clone(), self.theme.clone(), request, cx);
        }
    }

    fn popup_confirmation(
        &self,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let popup = self.pending_popups.first()?;
        let origin = popup.origin.clone();
        let panel = modal_panel(palette)
            .id("popup-confirmation-panel")
            .role(Role::AlertDialog)
            .aria_label("Allow pop-up window")
            .aria_description(format!("{} requested a separate window", origin))
            .aria_modal(true)
            .track_focus(&self.confirmation_focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.modifiers.modified() {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "escape" => this.block_pending_popup(window, cx),
                    "enter" => this.allow_pending_popup(window, cx),
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .child(
                div()
                    .text_size(px(metrics::MENU_FONT_SIZE))
                    .child("Allow this page to open a separate window?"),
            )
            .child(
                div()
                    .text_size(px(metrics::TAB_FONT_SIZE))
                    .text_color(rgb(palette.text_secondary))
                    .truncate()
                    .child(origin),
            )
            .child(
                super::super::layout::h_stack()
                    .items_center()
                    .justify_end()
                    .gap(px(metrics::BUTTON_GAP))
                    .child(super::super::button::button(
                        "popup-confirmation-block",
                        "Block",
                        false,
                        super::super::button::ButtonSize::Regular,
                        palette,
                        Box::new(cx.listener(|this, _, window, cx| {
                            cx.stop_propagation();
                            this.block_pending_popup(window, cx);
                        })),
                    ))
                    .child(super::super::button::button(
                        "popup-confirmation-allow",
                        "Open Window",
                        true,
                        super::super::button::ButtonSize::Regular,
                        palette,
                        Box::new(cx.listener(|this, _, window, cx| {
                            cx.stop_propagation();
                            this.allow_pending_popup(window, cx);
                        })),
                    )),
            );
        Some(modal(
            "popup-confirmation",
            Transition::Enter,
            palette,
            panel,
        ))
    }

    fn allow_pending_popup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.pending_popups.is_empty() {
            let popup = self.pending_popups.remove(0);
            open_minimal_window(self.runtime.clone(), self.theme.clone(), popup.request, cx);
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
        let appearance = self.theme.appearance(window.appearance());
        let palette = ThemeColors::for_appearance(appearance);
        let url = self.webview.read(cx).state.url.clone();
        v_stack()
            .size_full()
            .relative()
            .bg(rgba(palette.window_tint))
            .text_color(rgb(palette.text_primary))
            .child(titlebar(
                div()
                    .w_full()
                    .text_size(px(metrics::POPUP_URL_FONT_SIZE))
                    .text_color(rgb(palette.text_secondary))
                    .truncate()
                    .child(url),
            ))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .w_full()
                    .p(px(metrics::PAGE_INSET))
                    .child(self.webview.clone()),
            )
            .children(self.popup_confirmation(palette, cx))
    }
}
