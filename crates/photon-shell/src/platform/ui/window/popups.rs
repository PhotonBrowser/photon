//! Confirmation and routing for site-requested browsing contexts.

use gpui::{AnyElement, Context, Entity, FocusHandle, KeyDownEvent, Role, Window, prelude::*, px};

use crate::platform::engine::RequestedWebView;

use super::super::button::{ButtonKind, ButtonSize, button};
use super::super::layout::{h_stack, secondary_text};
use super::super::modal::{MODAL_MOTION, dialog_title, modal, modal_panel};
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
        &mut self,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let animations = super::super::settings::Settings::motion(cx);
        let origin = self
            .pending_popups
            .front()
            .map(|popup| popup.origin.clone());
        let (origin, transition) =
            self.popup_presence
                .sync(origin, MODAL_MOTION, &animations, cx)?;
        Some(popup_confirmation_dialog(
            origin,
            transition,
            &self.popup_confirmation_focus,
            palette,
            Self::block_pending_popup,
            Self::allow_pending_popup,
            cx,
        ))
    }

    pub(super) fn allow_pending_popup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(popup) = self.pending_popups.pop_front() {
            open_minimal_window(self.runtime.clone(), popup.request, cx);
        }
        self.refocus_after_popup_decision(window, cx);
    }

    pub(super) fn block_pending_popup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

/// Asks whether a page at `origin` may open a separate window. Enter allows
/// it and Escape blocks it. Browser and site windows both ask this way.
pub(super) fn popup_confirmation_dialog<V: 'static>(
    origin: String,
    transition: Transition,
    focus: &FocusHandle,
    palette: ThemeColors,
    block: fn(&mut V, &mut Window, &mut Context<V>),
    allow: fn(&mut V, &mut Window, &mut Context<V>),
    cx: &mut Context<V>,
) -> AnyElement {
    let panel = modal_panel(palette)
        .id("popup-confirmation-panel")
        .role(Role::AlertDialog)
        .aria_label("Allow pop-up window")
        .aria_description(format!("{origin} requested a separate window"))
        .aria_modal(true)
        .track_focus(focus)
        .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
            if event.keystroke.modifiers.modified() {
                return;
            }
            match event.keystroke.key.as_str() {
                "escape" => block(this, window, cx),
                "enter" => allow(this, window, cx),
                _ => return,
            }
            cx.stop_propagation();
        }))
        .child(dialog_title(
            "Allow this page to open a separate window?",
            palette,
        ))
        .child(secondary_text(origin, palette).truncate())
        .child(
            h_stack()
                .items_center()
                .justify_end()
                .gap(px(metrics::BUTTON_GAP))
                .child(button(
                    "popup-confirmation-block",
                    "Block",
                    ButtonKind::Secondary,
                    true,
                    ButtonSize::Regular,
                    palette,
                    Box::new(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        block(this, window, cx);
                    })),
                ))
                .child(button(
                    "popup-confirmation-allow",
                    "Open window",
                    ButtonKind::Primary,
                    true,
                    ButtonSize::Regular,
                    palette,
                    Box::new(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        allow(this, window, cx);
                    })),
                )),
        );
    modal("popup-confirmation", transition, palette, panel, cx).into_any_element()
}
