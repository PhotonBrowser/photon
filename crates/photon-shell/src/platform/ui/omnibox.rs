//! Address and search input in the native browser toolbar.

use super::super::trace;
use super::icons::search_icon_sized;
use super::layout::h_stack;
use super::{PhotonWebView, WebViewEvent};
use super::{metrics, theme::ThemeColors};
use gpui::{
    Context, Entity, Focusable, MouseButton, Render, Subscription, Window, prelude::*, px, rgb,
    rgb_to_hsla, rgba,
};
use gpui_elements::editable_text::{
    EditableTextState, StringStorage, TextChanged,
    actions::{Enter, Escape},
    text_input,
};

const INVALID_ADDRESS_DESCRIPTION: &str = "This address can't be opened";

pub(super) struct Omnibox {
    input: Entity<EditableTextState>,
    webview: Entity<PhotonWebView>,
    /// The submitted text cannot be opened. Cleared by the next edit.
    invalid: bool,
    _input_subscription: Subscription,
    _subscriptions: Vec<Subscription>,
}

impl Omnibox {
    pub(super) fn new(
        webview: Entity<PhotonWebView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| EditableTextState::new(StringStorage::default(), cx));
        let input_focus = input.focus_handle(cx);
        let subscriptions = vec![
            // Follow the page address, except while someone is editing it.
            cx.subscribe_in(&webview, window, |this, _, _: &WebViewEvent, window, cx| {
                if !this.is_editing(window, cx) {
                    this.show_page_url(cx);
                }
            }),
            // An abandoned edit reverts to the page address.
            cx.on_blur(&input_focus, window, |this, _, cx| this.show_page_url(cx)),
            cx.observe_window_appearance(window, |_, _, cx| cx.notify()),
        ];
        let input_subscription = cx.subscribe(&input, |this, _, _: &TextChanged, cx| {
            if this.invalid {
                this.invalid = false;
                cx.notify();
            }
        });
        let omnibox = Self {
            input,
            webview,
            invalid: false,
            _input_subscription: input_subscription,
            _subscriptions: subscriptions,
        };
        omnibox.show_page_url(cx);
        omnibox
    }

    pub(super) fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.input.focus_handle(cx), cx);
        self.input.update(cx, |input, cx| input.select_document(cx));
    }

    pub(super) fn set_webview(
        &mut self,
        webview: Entity<PhotonWebView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.webview = webview;
        let input_focus = self.input.focus_handle(cx);
        self._subscriptions = vec![
            cx.subscribe_in(
                &self.webview,
                window,
                |this, _, _: &WebViewEvent, window, cx| {
                    if !this.is_editing(window, cx) {
                        this.show_page_url(cx);
                    }
                },
            ),
            cx.on_blur(&input_focus, window, |this, _, cx| this.show_page_url(cx)),
            cx.observe_window_appearance(window, |_, _, cx| cx.notify()),
        ];
        self.show_page_url(cx);
        cx.notify();
    }

    fn is_editing(&self, window: &Window, cx: &Context<Self>) -> bool {
        self.input.focus_handle(cx).is_focused(window)
    }

    fn show_page_url(&self, cx: &mut Context<Self>) {
        let url = self.webview.read(cx).omnibox_url();
        self.input.update(cx, |input, cx| {
            if input.as_str() != url {
                input.emplace(&url, cx);
            }
        });
    }

    fn submit(&mut self, _: &Enter, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        let text = self.input.read(cx).as_str().to_owned();
        if text.trim().is_empty() {
            return;
        }
        match self.webview.update(cx, |view, cx| view.navigate(&text, cx)) {
            Ok(()) => self.return_to_page(window, cx),
            Err(error) => {
                // Keep the text for correction and say why nothing opened.
                trace(format_args!("omnibox: {error:#}"));
                self.invalid = true;
                cx.notify();
            }
        }
    }

    fn cancel(&mut self, _: &Escape, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        self.return_to_page(window, cx);
    }

    fn return_to_page(&self, window: &mut Window, cx: &mut Context<Self>) {
        let page_focus = self.webview.read(cx).focus_handle.clone();
        window.focus(&page_focus, cx);
    }
}

impl Render for Omnibox {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let appearance = self.webview.read(cx).theme.appearance(window.appearance());
        let palette = ThemeColors::for_appearance(appearance);
        let editing = self.is_editing(window, cx);
        let field = if editing {
            palette.field_focused
        } else {
            palette.field
        };
        let input_focus = self.input.focus_handle(cx).tab_index(3).tab_stop(true);
        let mut field_box = h_stack()
            .id("titlebar-omnibox")
            .items_center()
            .gap(px(metrics::OMNIBOX_GAP))
            .flex_1()
            .min_w(px(0.0))
            .h(px(metrics::OMNIBOX_HEIGHT))
            .px(px(metrics::OMNIBOX_HORIZONTAL_PADDING))
            .rounded(px(metrics::OMNIBOX_RADIUS))
            .bg(rgba(field))
            .border_1()
            .border_color(if self.invalid {
                rgb_to_hsla(rgba(palette.field_error_border))
            } else {
                gpui::transparent_black()
            })
            .text_size(px(metrics::OMNIBOX_FONT_SIZE))
            .text_color(rgb(palette.text_primary))
            // Clicks on the field's padding or icon edit the address rather than
            // falling through to the titlebar and moving the window.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    cx.stop_propagation();
                    window.focus(&this.input.focus_handle(cx), cx);
                }),
            )
            .capture_action(cx.listener(Self::submit))
            .capture_action(cx.listener(Self::cancel))
            .child(search_icon_sized(
                palette.text_primary,
                metrics::OMNIBOX_ICON_SIZE,
            ))
            .child(
                text_input("titlebar-omnibox-input")
                    .state(self.input.downgrade())
                    .track_focus(&input_focus)
                    .placeholder("Search or enter address")
                    .placeholder_color(rgb(palette.text_primary))
                    .caret_color(rgb_to_hsla(rgb(palette.accent)))
                    .selection_color(rgb_to_hsla(rgba(palette.selection)))
                    .caret_blink_interval_500ms()
                    .flex_1()
                    .min_w_0()
                    .whitespace_nowrap()
                    .overflow_x_scroll(),
            );
        if self.invalid {
            field_box = field_box.aria_description(INVALID_ADDRESS_DESCRIPTION);
        }
        field_box
    }
}
