//! Confirmation and routing for site-requested browsing contexts.

use gpui::{Context, Entity, KeyDownEvent, Role, Window, div, prelude::*, px, rgb};

use crate::platform::engine::RequestedWebView;

use super::super::button::{ButtonSize, button};
use super::super::layout::h_stack;
use super::super::modal::{modal, modal_panel};
use super::super::motion::Transition;
use super::super::{PhotonWebView, metrics, theme::ThemeColors};
use super::BrowserWindow;
use super::popup_window::open_minimal_window;

pub(super) struct PendingPopup {
    request: RequestedWebView,
    origin: String,
}

impl BrowserWindow {
    pub(super) fn handle_requested_web_view(
        &mut self,
        source: &Entity<PhotonWebView>,
        request: RequestedWebView,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if request.popup && request.needs_confirmation {
            let origin = source.read(cx).state.url.clone();
            let was_empty = self.pending_popups.is_empty();
            self.pending_popups
                .push_back(PendingPopup { request, origin });
            self.open_menu = None;
            if was_empty {
                window.focus(&self.popup_confirmation_focus, cx);
            }
            cx.notify();
        } else if request.popup {
            open_minimal_window(self.runtime.clone(), request, cx);
        } else {
            self.open_requested_tab(request, window, cx);
        }
    }

    pub(super) fn popup_confirmation(
        &self,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let popup = self.pending_popups.front()?;
        let origin = popup.origin.clone();
        let focus_handle = &self.popup_confirmation_focus;
        let panel = modal_panel(palette)
            .id("popup-confirmation-panel")
            .role(Role::AlertDialog)
            .aria_label("Allow pop-up window")
            .aria_description(format!("{} requested a separate window", origin))
            .aria_modal(true)
            .track_focus(focus_handle)
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
                h_stack()
                    .items_center()
                    .justify_end()
                    .gap(px(metrics::BUTTON_GAP))
                    .child(button(
                        "popup-confirmation-block",
                        "Block",
                        false,
                        ButtonSize::Regular,
                        palette,
                        Box::new(cx.listener(|this, _, window, cx| {
                            cx.stop_propagation();
                            this.block_pending_popup(window, cx);
                        })),
                    ))
                    .child(button(
                        "popup-confirmation-allow",
                        "Open Window",
                        true,
                        ButtonSize::Regular,
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
        if let Some(popup) = self.pending_popups.pop_front() {
            open_minimal_window(self.runtime.clone(), popup.request, cx);
        }
        self.refocus_after_popup_decision(window, cx);
    }

    fn block_pending_popup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pending_popups.pop_front();
        self.refocus_after_popup_decision(window, cx);
    }

    fn refocus_after_popup_decision(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending_popups.is_empty() {
            if let Some(webview) = self.active_webview() {
                let focus = webview.read(cx).focus_handle.clone();
                window.focus(&focus, cx);
            } else {
                self.omnibox
                    .update(cx, |omnibox, cx| omnibox.focus(window, cx));
            }
        } else {
            window.focus(&self.popup_confirmation_focus, cx);
        }
        cx.notify();
    }
}
