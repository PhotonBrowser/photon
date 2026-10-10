//! A page's JavaScript alert, confirm or prompt, shown over the page.

use gpui::{
    App, Context, Entity, FocusHandle, Focusable, KeyDownEvent, Render, Role, Subscription, Window,
    div, prelude::*, px, rgb, rgba,
};
use gpui_elements::editable_text::{
    EditableTextState, StringStorage,
    actions::{Enter, Escape},
    text_input,
};
use photon_core::{DialogKind, DialogRequest};

use super::button::{ButtonSize, button};
use super::controls::themed_text_input;
use super::layout::h_stack;
use super::modal::{modal, modal_panel};
use super::motion::Transition;
use super::{PhotonWebView, metrics, theme::palette};

pub(super) struct JavaScriptDialog {
    webview: Entity<PhotonWebView>,
    request: DialogRequest,
    /// The prompt's text field; alerts and confirms have none.
    input: Option<Entity<EditableTextState>>,
    focus_handle: FocusHandle,
    /// Whether the dialog is open or closing after being answered.
    transition: Transition,
    _input_subscription: Option<Subscription>,
}

impl JavaScriptDialog {
    /// Marks the dialog answered, so it animates away while the window keeps
    /// it drawn.
    pub(super) fn leave(&mut self, cx: &mut Context<Self>) {
        self.transition = Transition::Exit;
        cx.notify();
    }

    pub(super) fn new(
        webview: Entity<PhotonWebView>,
        request: DialogRequest,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = match &request.kind {
            DialogKind::Prompt { default_text } => Some(cx.new(|cx| {
                let mut input = EditableTextState::new(StringStorage::default(), cx);
                input.emplace(default_text, cx);
                input
            })),
            DialogKind::Alert | DialogKind::Confirm => None,
        };
        // The window draws the dialog only when a view inside it is notified.
        let input_subscription = input
            .as_ref()
            .map(|input| cx.observe(input, |_, _, cx| cx.notify()));
        Self {
            webview,
            request,
            input,
            focus_handle: cx.focus_handle(),
            transition: Transition::Enter,
            _input_subscription: input_subscription,
        }
    }

    pub(super) fn request(&self) -> &DialogRequest {
        &self.request
    }

    /// Focuses the prompt's text, selected, or the dialog itself.
    pub(super) fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        match &self.input {
            Some(input) => {
                window.focus(&input.focus_handle(cx), cx);
                input.update(cx, |input, cx| input.select_document(cx));
            }
            None => window.focus(&self.focus_handle, cx),
        }
    }

    pub(super) fn contains_focus(&self, window: &Window, cx: &App) -> bool {
        self.focus_handle.contains_focused(window, cx)
            || self
                .input
                .as_ref()
                .is_some_and(|input| input.focus_handle(cx).is_focused(window))
    }

    fn close(&mut self, accepted: bool, cx: &mut Context<Self>) {
        let text = self
            .input
            .as_ref()
            .map(|input| input.read(cx).as_str().to_owned())
            .unwrap_or_default();
        self.webview
            .update(cx, |view, cx| view.close_dialog(accepted, text, cx));
    }

    fn accept_from_field(&mut self, _: &Enter, _: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        self.close(true, cx);
    }

    fn cancel_from_field(&mut self, _: &Escape, _: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        self.close(false, cx);
    }
}

impl Render for JavaScriptDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = palette(window, cx);
        let is_alert = self.request.kind == DialogKind::Alert;

        let mut buttons = h_stack()
            .items_center()
            .justify_end()
            .gap(px(metrics::BUTTON_GAP));
        if !is_alert {
            buttons = buttons.child(button(
                "javascript-dialog-cancel",
                "Cancel",
                false,
                ButtonSize::Regular,
                palette,
                Box::new(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.close(false, cx);
                })),
            ));
        }
        buttons = buttons.child(button(
            "javascript-dialog-ok",
            "OK",
            true,
            ButtonSize::Regular,
            palette,
            Box::new(cx.listener(|this, _, _, cx| {
                cx.stop_propagation();
                this.close(true, cx);
            })),
        ));

        let mut panel = modal_panel(palette)
            .id("javascript-dialog")
            .role(Role::AlertDialog)
            .aria_label(self.request.title.clone())
            .aria_description(self.request.message.clone())
            .aria_modal(true)
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.modifiers.modified() {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "enter" => this.close(true, cx),
                    "escape" => this.close(false, cx),
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .child(
                div()
                    .text_size(px(metrics::TAB_FONT_SIZE))
                    .text_color(rgb(palette.text_secondary))
                    .truncate()
                    .child(self.request.title.clone()),
            )
            .child(
                div()
                    .id("javascript-dialog-message")
                    .max_h(px(metrics::DIALOG_MESSAGE_MAX_HEIGHT))
                    .overflow_y_scroll()
                    .text_size(px(metrics::MENU_FONT_SIZE))
                    .child(self.request.message.clone()),
            );

        if let Some(input) = &self.input {
            panel = panel.child(
                h_stack()
                    .items_center()
                    .h(px(metrics::OMNIBOX_HEIGHT))
                    .px(px(metrics::OMNIBOX_HORIZONTAL_PADDING))
                    .rounded(px(metrics::MENU_ITEM_RADIUS))
                    .bg(rgba(palette.surface))
                    .text_size(px(metrics::MENU_FONT_SIZE))
                    .capture_action(cx.listener(Self::accept_from_field))
                    .capture_action(cx.listener(Self::cancel_from_field))
                    .child(
                        themed_text_input(text_input("javascript-dialog-input"), palette)
                            .state(input.downgrade())
                            .track_focus(&input.focus_handle(cx))
                            .flex_1()
                            .min_w_0()
                            .whitespace_nowrap()
                            .overflow_x_scroll(),
                    ),
            );
        }

        // The page waits for an answer, so the modal blocks it until then.
        modal(
            "javascript-dialog",
            self.transition,
            palette,
            panel.child(buttons),
        )
    }
}
