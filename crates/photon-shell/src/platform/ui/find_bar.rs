//! The find-in-page bar, floating in the page's top-right corner.

use gpui::{
    Context, Entity, EventEmitter, Focusable, KeyDownEvent, MouseButton, Render, Role,
    Subscription, Window, div, prelude::*, px, rgb, rgb_to_hsla, rgba,
};
use gpui_elements::editable_text::{
    EditableTextState, StringStorage, TextChanged,
    actions::{Enter, Escape},
    text_input,
};

use super::button::icon_button;
use super::icons::{chevron_down_icon, chevron_up_icon, close_icon, search_icon_sized};
use super::layout::h_stack;
use super::motion::Entrance;
use super::{PhotonWebView, WebViewEvent, metrics, theme::ThemeColors};

/// How the bar drops in and lifts away.
pub(super) const FIND_BAR_MOTION: Entrance = Entrance::fall();

/// Asks the window to close the bar.
pub(super) struct CloseFindBar;

pub(super) struct FindBar {
    webview: Entity<PhotonWebView>,
    input: Entity<EditableTextState>,
    _subscriptions: [Subscription; 3],
}

impl EventEmitter<CloseFindBar> for FindBar {}

impl FindBar {
    pub(super) fn new(webview: Entity<PhotonWebView>, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| EditableTextState::new(StringStorage::default(), cx));
        let subscriptions = [
            // Search as the query changes.
            cx.subscribe(&input, |this, input, _: &TextChanged, cx| {
                let query = input.read(cx).as_str().to_owned();
                this.webview.update(cx, |view, cx| view.find(&query, cx));
            }),
            // Redraw for every edit, caret move and blink.
            cx.observe(&input, |_, _, cx| cx.notify()),
            // Redraw when the page reports a new result.
            cx.subscribe(&webview, |_, _, _: &WebViewEvent, cx| cx.notify()),
        ];
        Self {
            webview,
            input,
            _subscriptions: subscriptions,
        }
    }

    /// Focuses the query, selected, so typing replaces it.
    pub(super) fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.input.focus_handle(cx), cx);
        self.input.update(cx, |input, cx| input.select_document(cx));
    }

    /// Moves to the next match, or the previous one when `forward` is false.
    pub(super) fn step(&mut self, forward: bool, cx: &mut Context<Self>) {
        self.webview.update(cx, |view, _| view.find_step(forward));
    }

    fn next(&mut self, _: &Enter, _: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        self.step(true, cx);
    }

    fn close(&mut self, _: &Escape, _: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        cx.emit(CloseFindBar);
    }

    /// Shift-Enter goes back a match; Escape closes the bar from its buttons.
    fn key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let modifiers = event.keystroke.modifiers;
        match event.keystroke.key.as_str() {
            "enter" if modifiers.shift && !modifiers.platform => self.step(false, cx),
            "escape" => cx.emit(CloseFindBar),
            _ => return,
        }
        cx.stop_propagation();
    }

    /// "3 of 12", "No matches", or nothing before the page has counted.
    fn result_label(&self, cx: &Context<Self>) -> Option<String> {
        if self.input.read(cx).as_str().is_empty() {
            return None;
        }
        let result = self.webview.read(cx).find_result?;
        Some(match result.total_match_count? {
            0 => "No matches".to_owned(),
            total => format!("{} of {total}", result.current_match_index + 1),
        })
    }
}

impl Render for FindBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let appearance = self.webview.read(cx).theme.appearance(window.appearance());
        let palette = ThemeColors::for_appearance(appearance);
        let has_matches = self
            .webview
            .read(cx)
            .find_result
            .and_then(|result| result.total_match_count)
            .is_some_and(|total| total > 0);
        let icon_size = metrics::ICON_BUTTON_ICON_SIZE;

        h_stack()
            .id("find-bar")
            .role(Role::Search)
            .aria_label("Find in page")
            .items_center()
            .gap(px(metrics::FIND_BAR_GAP))
            .h(px(metrics::FIND_BAR_HEIGHT))
            .pl(px(metrics::FIND_BAR_PADDING))
            .pr(px(metrics::FIND_BAR_TRAILING_PADDING))
            .rounded(px(metrics::SURFACE_RADIUS))
            .border_1()
            .border_color(rgba(palette.menu_border))
            .bg(rgba(palette.menu_surface))
            .text_size(px(metrics::FIND_FONT_SIZE))
            .text_color(rgb(palette.text_primary))
            .shadow_md()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .capture_action(cx.listener(Self::next))
            .capture_action(cx.listener(Self::close))
            .on_key_down(cx.listener(Self::key_down))
            .child(search_icon_sized(palette.text_secondary, icon_size))
            .child(
                text_input("find-bar-input")
                    .state(self.input.downgrade())
                    .track_focus(&self.input.focus_handle(cx))
                    .placeholder("Find in page")
                    .placeholder_color(rgb(palette.text_secondary))
                    .caret_color(rgb_to_hsla(rgb(palette.accent)))
                    .selection_color(rgb_to_hsla(rgba(palette.selection)))
                    .caret_blink_interval_500ms()
                    .w(px(metrics::FIND_FIELD_WIDTH))
                    .whitespace_nowrap()
                    .overflow_x_scroll(),
            )
            .children(self.result_label(cx).map(|label| {
                div()
                    .flex_shrink_0()
                    .text_size(px(metrics::FIND_RESULT_FONT_SIZE))
                    .text_color(rgb(palette.text_secondary))
                    .child(label)
            }))
            .child(icon_button(
                "find-bar-previous",
                "Previous match",
                has_matches,
                metrics::ICON_BUTTON_SIZE,
                chevron_up_icon(palette.text_primary, icon_size),
                palette,
                Box::new(cx.listener(|this, _, _, cx| this.step(false, cx))),
            ))
            .child(icon_button(
                "find-bar-next",
                "Next match",
                has_matches,
                metrics::ICON_BUTTON_SIZE,
                chevron_down_icon(palette.text_primary, icon_size),
                palette,
                Box::new(cx.listener(|this, _, _, cx| this.step(true, cx))),
            ))
            .child(icon_button(
                "find-bar-close",
                "Close find bar",
                true,
                metrics::ICON_BUTTON_SIZE,
                close_icon(palette.text_secondary, icon_size),
                palette,
                Box::new(cx.listener(|_, _, _, cx| cx.emit(CloseFindBar))),
            ))
    }
}
