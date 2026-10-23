//! Window presentation for requests managed by the reusable permission
//! coordinator.

use gpui::{AnyElement, App, Context, FocusHandle, KeyDownEvent, Role, Window, prelude::*, px};

use crate::platform::permission_coordinator::PermissionRequest;

use super::super::button::{ButtonKind, ButtonSize, button};
use super::super::layout::{h_stack, secondary_text};
use super::super::modal::{MODAL_MOTION, dialog_title, modal, modal_panel};
use super::super::motion::Transition;
use super::super::{PhotonWebView, metrics, theme::ThemeColors};
use super::BrowserWindow;

pub(super) struct PermissionPromptEntry {
    token: u64,
    webview: gpui::WeakEntity<PhotonWebView>,
}

impl BrowserWindow {
    pub(in crate::platform) fn queue_permission_request(
        &mut self,
        request: PermissionRequest,
        webview: gpui::WeakEntity<PhotonWebView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(token) = self.permission_coordinator.begin(request) else {
            return false;
        };
        let first = self.permission_prompts.is_empty();
        self.permission_prompts
            .push_back(PermissionPromptEntry { token, webview });
        if first {
            window.focus(&self.permission_focus, cx);
        }
        cx.notify();
        true
    }

    pub(super) fn permission_prompt(
        &mut self,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let entry = self.permission_prompts.front()?;
        let request = self.permission_coordinator.get(entry.token)?.clone();
        let animations = super::super::settings::Settings::motion(cx);
        let (_, transition) =
            self.permission_presence
                .sync(Some(entry.token), MODAL_MOTION, &animations, cx)?;
        let embedded_site = (request.requesting_origin != request.top_level_origin)
            .then(|| display_origin(&request.top_level_origin));
        Some(permission_prompt_dialog(
            transition,
            PermissionPromptContent {
                requesting_site: display_origin(&request.requesting_origin),
                embedded_site,
            },
            &self.permission_focus,
            palette,
            Self::deny_permission_prompt,
            Self::allow_permission_once,
            cx,
        ))
    }

    pub(super) fn allow_permission_once(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.resolve_permission_prompt(true, window, cx);
    }

    pub(super) fn deny_permission_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.resolve_permission_prompt(false, window, cx);
    }

    fn resolve_permission_prompt(
        &mut self,
        granted: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(entry) = self.permission_prompts.pop_front() else {
            return;
        };
        if let Some(request) = self.permission_coordinator.resolve(entry.token) {
            entry
                .webview
                .update(cx, |view, _| {
                    view.answer_permission_request(
                        request.engine_request_id,
                        &request.document_id,
                        granted,
                    );
                })
                .ok();
        }
        self.refocus_permission_owner(window, cx);
        cx.notify();
    }

    pub(in crate::platform) fn cancel_permission_request(
        &mut self,
        view_id: u64,
        request_id: u64,
        document_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((token, _)) =
            self.permission_coordinator
                .cancel_request(view_id, request_id, document_id)
        else {
            return;
        };
        self.permission_prompts.retain(|entry| entry.token != token);
        self.refocus_permission_owner(window, cx);
        cx.notify();
    }

    pub(super) fn cancel_permissions_for_view(
        &mut self,
        view_id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let cancelled = self.permission_coordinator.cancel_view(view_id);
        if cancelled.is_empty() {
            return;
        }
        let tokens: std::collections::HashSet<_> =
            cancelled.iter().map(|(token, _)| *token).collect();
        let entries = std::mem::take(&mut self.permission_prompts);
        for entry in entries {
            if tokens.contains(&entry.token) {
                if let Some((_, request)) =
                    cancelled.iter().find(|(token, _)| *token == entry.token)
                {
                    let request = request.clone();
                    entry
                        .webview
                        .update(cx, |view, _| {
                            view.answer_permission_request(
                                request.engine_request_id,
                                &request.document_id,
                                false,
                            );
                        })
                        .ok();
                }
            } else {
                self.permission_prompts.push_back(entry);
            }
        }
        self.refocus_permission_owner(window, cx);
        cx.notify();
    }

    pub(super) fn deny_all_permission_requests(&mut self, cx: &mut App) {
        let cancelled = self.permission_coordinator.cancel_all();
        let entries = std::mem::take(&mut self.permission_prompts);
        for entry in entries {
            let Some((_, request)) = cancelled.iter().find(|(token, _)| *token == entry.token)
            else {
                continue;
            };
            let request = request.clone();
            entry
                .webview
                .update(cx, |view, _| {
                    view.answer_permission_request(
                        request.engine_request_id,
                        &request.document_id,
                        false,
                    );
                })
                .ok();
        }
    }

    fn refocus_permission_owner(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.permission_prompts.is_empty() {
            window.focus(&self.permission_focus, cx);
        } else if let Some(webview) = self.active_webview() {
            let focus_handle = webview.read(cx).focus_handle.clone();
            window.focus(&focus_handle, cx);
        } else {
            self.omnibox
                .update(cx, |omnibox, cx| omnibox.focus(window, cx));
        }
    }
}

fn display_origin(origin: &str) -> String {
    origin
        .strip_prefix("https://")
        .or_else(|| origin.strip_prefix("http://"))
        .unwrap_or(origin)
        .trim_end_matches('/')
        .to_owned()
}

struct PermissionPromptContent {
    requesting_site: String,
    embedded_site: Option<String>,
}

fn permission_prompt_dialog<V: 'static>(
    transition: Transition,
    content: PermissionPromptContent,
    focus: &FocusHandle,
    palette: ThemeColors,
    deny: fn(&mut V, &mut Window, &mut Context<V>),
    allow_once: fn(&mut V, &mut Window, &mut Context<V>),
    cx: &mut Context<V>,
) -> AnyElement {
    let PermissionPromptContent {
        requesting_site,
        embedded_site,
    } = content;
    let panel = modal_panel(palette)
        .id("permission-prompt-panel")
        .role(Role::AlertDialog)
        .aria_label("Microphone access")
        .aria_description(format!("{requesting_site} wants to use your microphone"))
        .aria_modal(true)
        .track_focus(focus)
        .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
            if event.keystroke.modifiers.modified() {
                return;
            }
            match event.keystroke.key.as_str() {
                "escape" => deny(this, window, cx),
                "enter" => allow_once(this, window, cx),
                _ => return,
            }
            cx.stop_propagation();
        }))
        .child(dialog_title("Microphone access", palette))
        .child(
            secondary_text(
                format!("{requesting_site} wants to use your microphone."),
                palette,
            )
            .text_size(px(metrics::DIALOG_TEXT_SIZE)),
        )
        .children(embedded_site.map(|site| {
            secondary_text(
                format!("Requested from a frame embedded on {site}"),
                palette,
            )
        }))
        .child(
            h_stack()
                .items_center()
                .justify_end()
                .gap(px(metrics::BUTTON_GAP))
                .child(button(
                    "permission-deny",
                    "Don't Allow",
                    ButtonKind::Secondary,
                    true,
                    ButtonSize::Regular,
                    palette,
                    Box::new(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        deny(this, window, cx);
                    })),
                ))
                .child(button(
                    "permission-allow-once",
                    "Allow Once",
                    ButtonKind::Primary,
                    true,
                    ButtonSize::Regular,
                    palette,
                    Box::new(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        allow_once(this, window, cx);
                    })),
                )),
        );
    modal("permission-prompt", transition, palette, panel, cx).into_any_element()
}
