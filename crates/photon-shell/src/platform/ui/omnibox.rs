//! The titlebar omnibox: shows the page address and opens typed addresses or searches.

use gpui::{
    Context, Entity, Focusable, MouseButton, Render, Subscription, Window, div, prelude::*, px, rgb,
    rgb_to_hsla, rgba,
};
use gpui_elements::editable_text::{
    EditableTextState, StringStorage,
    actions::{Enter, Escape},
    text_input,
};
use super::super::trace;
use super::PhotonWebView;
use super::icons::{loading_spinner, search_icon};
use super::theme::{Palette, metrics};

gpui::actions!(photon, [
    /// Move keyboard focus to the omnibox and select its contents.
    FocusOmnibox
]);

pub(super) struct Omnibox {
    input: Entity<EditableTextState>,
    webview: Entity<PhotonWebView>,
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
            cx.observe_in(&webview, window, move |this, _, window, cx| {
                if !this.is_editing(window, cx) {
                    this.show_page_url(cx);
                }
            }),
            // An abandoned edit reverts to the page address.
            cx.on_blur(&input_focus, window, |this, _, cx| this.show_page_url(cx)),
            cx.observe_window_appearance(window, |_, _, cx| cx.notify()),
        ];
        let omnibox = Self {
            input,
            webview,
            _subscriptions: subscriptions,
        };
        omnibox.show_page_url(cx);
        omnibox
    }

    pub(super) fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.input.focus_handle(cx), cx);
        self.input.update(cx, |input, cx| input.select_document(cx));
    }

    fn is_editing(&self, window: &Window, cx: &Context<Self>) -> bool {
        self.input.focus_handle(cx).is_focused(window)
    }

    fn show_page_url(&self, cx: &mut Context<Self>) {
        let url = self.webview.read(cx).url.clone();
        self.input.update(cx, |input, cx| {
            if input.as_str() != url {
                input.emplace(&url, cx);
            }
        });
    }

    fn submit(&mut self, _: &Enter, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        let text = self.input.read(cx).as_str().to_owned();
        match self
            .webview
            .update(cx, |view, _| view.session.navigate(&text))
        {
            Ok(()) => self.return_to_page(window, cx),
            Err(error) => trace(format_args!("omnibox: {error:#}")),
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
        let palette = Palette::for_appearance(window.appearance());
        let loading = self.webview.read(cx).loading;
        let field = if self.is_editing(window, cx) {
            palette.field_focused
        } else {
            palette.field
        };
        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .w_full()
            .max_w(px(metrics::OMNIBOX_MAX_WIDTH))
            .h(px(metrics::OMNIBOX_HEIGHT))
            .px(px(10.0))
            .rounded(px(8.0))
            .bg(rgba(field))
            .text_size(px(13.0))
            .text_color(rgb(palette.text))
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
            .child(if loading {
                loading_spinner(palette.accent).into_any_element()
            } else {
                search_icon(palette.text_muted).into_any_element()
            })
            .child(
                text_input("omnibox-input")
                    .state(self.input.downgrade())
                    .placeholder("Search or enter address")
                    .placeholder_color(rgb(palette.text_muted))
                    .caret_color(rgb_to_hsla(rgb(palette.accent)))
                    .selection_color(rgb_to_hsla(rgba(palette.selection)))
                    .caret_blink_interval_500ms()
                    .flex_1()
                    .min_w_0()
                    .whitespace_nowrap()
                    .overflow_x_scroll(),
            )
    }
}
