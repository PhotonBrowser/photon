//! Peek, as in Arc: a shift-clicked link opens in a preview over the page,
//! to read and close, or to keep as a tab without reloading.

use gpui::{
    AnyElement, Context, Entity, KeyDownEvent, MouseButton, Subscription, Window, div, prelude::*,
    px, relative, rgba,
};

use super::super::button::icon_button;
use super::super::controls::OwnsClicks;
use super::super::icons::{close_icon, open_in_tab_icon};
use super::super::layout::{Elevated, Elevation, Raised, h_stack, v_stack};
use super::super::motion::{AnimateIn, Entrance, Presence, fade, rise};
use super::super::{PhotonWebView, WebViewEvent, metrics, theme::ThemeColors};
use super::BrowserWindow;
use super::content::{create_webview, follow_appearance};

/// How the preview rises in, and leaves the same way.
const PEEK_MOTION: Entrance = rise();

/// The page shown in the preview, and the window following it.
pub(super) struct Peek {
    webview: Entity<PhotonWebView>,
    _subscription: Subscription,
}

/// The preview while open, kept drawn while it animates away.
#[derive(Default)]
pub(super) struct PeekState {
    open: Option<Peek>,
    presence: Presence<Entity<PhotonWebView>>,
}

impl BrowserWindow {
    /// Shows `url` in the preview, replacing a page already shown there.
    pub(super) fn open_peek(&mut self, url: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(webview) = create_webview(cx, self.runtime.clone(), url) else {
            return;
        };
        follow_appearance(&webview, window, cx);
        let subscription = cx.subscribe_in(
            &webview,
            window,
            |this, webview, event: &WebViewEvent, window, cx| match event {
                WebViewEvent::Peek(url) => this.open_peek(url, window, cx),
                WebViewEvent::OpenInNewTab { url, activate } => {
                    this.open_tab_beside(webview, url, *activate, window, cx);
                }
                WebViewEvent::StateChanged => {
                    this.sync_dialog(window, cx);
                    cx.notify();
                }
                WebViewEvent::ContextMenuRequested | WebViewEvent::NewWebViewRequested(_) => {}
            },
        );
        webview.update(cx, |view, _| view.session.set_visible(true));
        let focus = webview.read(cx).focus_handle.clone();
        window.focus(&focus, cx);
        self.peek.open = Some(Peek {
            webview,
            _subscription: subscription,
        });
        cx.notify();
    }

    /// The page in the preview, while it is open.
    pub(super) fn peek_webview(&self) -> Option<Entity<PhotonWebView>> {
        self.peek.open.as_ref().map(|peek| peek.webview.clone())
    }

    /// Closes the preview and returns to the page under it.
    pub(super) fn close_peek(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(peek) = self.peek.open.take() {
            peek.webview.update(cx, |view, _| {
                view.session.set_visible(false);
                view.session.set_focus(false);
            });
            if let Some(active_tab) = self.active_tab {
                self.activate_tab(active_tab, true, window, cx);
            }
            // A dialog the preview's page had open goes with it.
            self.sync_dialog(window, cx);
        }
    }

    /// Moves the previewed page, as it is, into a tab beside the one under it.
    fn keep_peek_as_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(peek) = self.peek.open.take() {
            let index = self.active_tab.map_or(0, |active_tab| active_tab + 1);
            self.insert_webview(index, peek.webview, None, true, window, cx);
        }
    }

    /// The preview over the window: the page in a raised card, with buttons
    /// to keep it as a tab or close it. A click outside or Escape closes it.
    pub(super) fn peek_overlay(
        &mut self,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let animations = super::super::settings::Settings::motion(cx);
        let shown = self.peek.open.as_ref().map(|peek| peek.webview.clone());
        let (webview, transition) = self
            .peek
            .presence
            .sync(shown, PEEK_MOTION, &animations, cx)?;
        let open = self.peek.open.is_some();
        // The buttons sit on a raised pill, to read over any page.
        let buttons = h_stack()
            .p(px(metrics::MENU_PADDING))
            .gap(px(metrics::PEEK_CONTROL_GAP))
            .rounded_full()
            .border_1()
            .border_color(rgba(palette.menu_border))
            .raised(palette)
            .owns_clicks()
            .child(icon_button(
                "peek-open-in-tab",
                "Open in tab",
                open,
                metrics::ICON_BUTTON_SIZE,
                open_in_tab_icon(palette.text_primary, metrics::ICON_BUTTON_ICON_SIZE),
                palette,
                Box::new(cx.listener(|this, _, window, cx| this.keep_peek_as_tab(window, cx))),
            ))
            .child(icon_button(
                "peek-close",
                "Close preview",
                open,
                metrics::ICON_BUTTON_SIZE,
                close_icon(palette.text_primary, metrics::ICON_BUTTON_ICON_SIZE),
                palette,
                Box::new(cx.listener(|this, _, window, cx| this.close_peek(window, cx))),
            ));
        let controls = h_stack().justify_end().child(buttons);
        let card = div()
            .flex_1()
            .min_h_0()
            .rounded(px(metrics::peek_card_radius()))
            .overflow_hidden()
            .border_1()
            .border_color(rgba(palette.menu_border))
            .elevated(Elevation::High)
            .owns_clicks()
            .child(webview);
        let panel = v_stack()
            .size_full()
            .gap(px(metrics::PEEK_CONTROL_GAP))
            .child(controls)
            .child(card)
            .animate("peek-panel", PEEK_MOTION, transition, &animations);
        let mut overlay = div()
            .id("peek")
            .absolute()
            .inset_0()
            .px(relative(metrics::PEEK_INSET_FRACTION))
            .py(relative(metrics::PEEK_INSET_FRACTION / 2.0))
            .bg(rgba(palette.peek_backdrop))
            .child(panel);
        // A closing preview no longer holds the pointer or keys.
        if open {
            overlay = overlay
                .occlude()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| {
                        this.close_peek(window, cx);
                    }),
                )
                .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    if event.keystroke.key == "escape" && !event.keystroke.modifiers.modified() {
                        cx.stop_propagation();
                        this.close_peek(window, cx);
                    }
                }));
        }
        Some(
            overlay
                .animate("peek-backdrop", fade(), transition, &animations)
                .into_any_element(),
        )
    }
}
