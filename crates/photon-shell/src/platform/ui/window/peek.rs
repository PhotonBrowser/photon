//! Peek, as in Arc: a shift-clicked link opens in a preview over the page,
//! to read and close, or to keep as a tab without reloading.

use gpui::{
    AnyElement, Bounds, Context, Entity, KeyDownEvent, MouseButton, Pixels, Point, Subscription,
    Window, bounds, div, point, prelude::*, px, rgb, rgba, size,
};
use std::time::Instant;

use super::super::button::icon_button;
use super::super::controls::OwnsClicks;
use super::super::icons::{close_icon, open_in_tab_icon};
use super::super::layout::{Elevated, Elevation, Raised, h_stack};
use super::super::motion::{PEEK_CLOSE_SPRING, PEEK_MORPH_SPRING, Speed, Tween, mix};
use super::super::{PhotonWebView, WebViewEvent, metrics, theme::ThemeColors};
use super::BrowserWindow;
use super::content::{create_webview, follow_appearance};

/// The page shown in the preview, and the window following it.
pub(super) struct Peek {
    webview: Entity<PhotonWebView>,
    /// Where the link was clicked, which the card grows out of and returns to.
    origin: Point<Pixels>,
    /// From 0 at the link to 1 at the card's frame. It starts with the first
    /// frame drawn, so the time spent creating the page is not skipped over.
    morph: Option<Tween>,
    _subscription: Subscription,
}

impl Peek {
    /// How far the card has grown, from 0 at the link to 1 on its frame.
    fn progress(&self, now: Instant) -> f32 {
        self.morph.as_ref().map_or(0.0, |morph| morph.value_at(now))
    }

    fn settled(&self, now: Instant) -> bool {
        self.morph
            .as_ref()
            .is_some_and(|morph| !morph.is_running_at(now))
    }
}

/// The preview while open, and while it shrinks back into its link.
#[derive(Default)]
pub(super) struct PeekState {
    open: Option<Peek>,
    closing: Option<Peek>,
}

impl BrowserWindow {
    /// Shows `url` in the preview, replacing a page already shown there.
    pub(super) fn open_peek(&mut self, url: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(webview) = create_webview(cx, self.runtime.clone(), url) else {
            return;
        };
        // A preview still shrinking away gives way to the new one.
        self.finish_closing_peek(cx);
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
                WebViewEvent::ContextMenuRequested
                | WebViewEvent::SelectDropdownRequested
                | WebViewEvent::NewWebViewRequested(_) => {}
                // A preview is too small to show a page fullscreen.
                WebViewEvent::FullscreenChanged(true) => {
                    webview.update(cx, |view, _| view.exit_page_fullscreen());
                }
                WebViewEvent::FullscreenChanged(false) => {}
            },
        );
        // Lay the page out at the card's final size now, so it loads and
        // lays out while the card is still growing, before its first frame.
        let page = peek_page_size(peek_frame(window));
        let scale = window.scale_factor();
        webview.update(cx, |view, _| {
            view.set_page_visible(true);
            view.set_layout_size(Some(page));
            view.adopt_viewport(Some((
                f32::from(page.width).round() as i32,
                f32::from(page.height).round() as i32,
                scale.to_bits(),
            )));
        });
        let focus = webview.read(cx).focus_handle.clone();
        window.focus(&focus, cx);
        self.peek.open = Some(Peek {
            webview,
            origin: window.mouse_position(),
            morph: None,
            _subscription: subscription,
        });
        cx.notify();
    }

    /// The page in the preview, while it is open.
    pub(super) fn peek_webview(&self) -> Option<Entity<PhotonWebView>> {
        self.peek.open.as_ref().map(|peek| peek.webview.clone())
    }

    /// Closes the preview and returns to the page under it. The card shrinks
    /// back into its link; the page under it takes the keyboard at once.
    pub(super) fn close_peek(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mut peek) = self.peek.open.take() else {
            return;
        };
        peek.webview
            .update(cx, |view, _| view.session.set_focus(false));
        // Only one preview shrinks away at a time.
        self.finish_closing_peek(cx);
        let animations = super::super::settings::Settings::motion(cx);
        let animate = animations.allows_motion() && !cx.reduce_motion();
        let now = Instant::now();
        let progress = peek.progress(now);
        if animate {
            peek.morph
                .get_or_insert_with(|| Tween::at_with_curve(progress, PEEK_CLOSE_SPRING))
                .animate_to_at(
                    0.0,
                    animations.duration(Speed::Quick),
                    PEEK_CLOSE_SPRING,
                    now,
                );
        }
        self.peek.closing = Some(peek);
        if !animate {
            self.finish_closing_peek(cx);
        }
        if let Some(active_tab) = self.active_tab {
            self.activate_tab(active_tab, true, window, cx);
        }
        // A dialog the preview's page had open goes with it.
        self.sync_dialog(window, cx);
        cx.notify();
    }

    /// Hides the page of a preview that has shrunk away.
    fn finish_closing_peek(&mut self, cx: &mut Context<Self>) {
        if let Some(peek) = self.peek.closing.take() {
            peek.webview.update(cx, |view, _| {
                view.set_layout_size(None);
                view.set_page_opacity(1.0);
                view.set_page_visible(false);
            });
        }
    }

    /// Moves the previewed page, as it is, into a tab beside the one under it.
    fn keep_peek_as_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(peek) = self.peek.open.take() {
            peek.webview.update(cx, |view, _| {
                view.set_layout_size(None);
                view.set_page_opacity(1.0);
            });
            let index = self.active_tab.map_or(0, |active_tab| active_tab + 1);
            self.insert_webview(index, peek.webview, None, true, window, cx);
        }
    }

    /// The preview over the window: the page in a raised card, with buttons
    /// to keep it as a tab or close it. A click outside or Escape closes it.
    ///
    /// The card grows out of the clicked link, keeping its final shape, and
    /// shrinks back into it on close. The page loads at the card's final size
    /// from the start and is scaled on the way, so it never lays out again.
    /// The backdrop and buttons follow the card's growth.
    pub(super) fn peek_overlay(
        &mut self,
        palette: ThemeColors,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let now = Instant::now();
        if self
            .peek
            .closing
            .as_ref()
            .is_some_and(|peek| peek.settled(now))
        {
            self.finish_closing_peek(cx);
        }
        let open = self.peek.open.is_some();
        let animations = super::super::settings::Settings::motion(cx);
        let reduce_motion = cx.reduce_motion();
        let peek = self.peek.open.as_mut().or(self.peek.closing.as_mut())?;
        peek.morph.get_or_insert_with(|| {
            let mut morph = Tween::at_with_curve(0.0, PEEK_MORPH_SPRING);
            if reduce_motion {
                morph.jump_to(1.0);
            } else {
                morph.animate_to(1.0, Speed::Gentle, &animations);
            }
            morph
        });
        let frame = peek_frame(window);
        let settled = peek.settled(now);
        if !settled {
            window.request_animation_frame();
        }
        let progress = peek.progress(now);
        // Once open and settled, exactly on the frame the page was laid out
        // for, not wherever the spring's last sample left it.
        let landed = open && settled;
        let card_bounds = if landed {
            frame
        } else {
            morph_bounds(peek.origin, frame, progress)
        };
        let layout_size = (!landed).then(|| peek_page_size(frame));
        // The bounce carries the card past its frame; the fades stop at full.
        let shown = progress.clamp(0.0, 1.0);
        // The card itself fades over the first part of its growth, and the
        // last part of its shrinking, so it never vanishes at full size or
        // pops in as a solid speck.
        let card_opacity = (shown * metrics::PEEK_CARD_FADE_RATE).min(1.0);
        let webview = peek.webview.clone();
        webview.update(cx, |view, _| {
            view.set_layout_size(layout_size);
            view.set_page_opacity(card_opacity);
        });

        let backdrop = div()
            .absolute()
            .inset_0()
            .bg(rgba(palette.peek_backdrop))
            .opacity(shown);
        let card = div()
            .absolute()
            .left(card_bounds.origin.x)
            .top(card_bounds.origin.y)
            .w(card_bounds.size.width)
            .h(card_bounds.size.height)
            .rounded(px(metrics::peek_card_radius()))
            .overflow_hidden()
            .border(px(metrics::SURFACE_BORDER_WIDTH))
            .border_color(rgba(palette.menu_border))
            .bg(rgb(palette.page_background))
            .elevated(Elevation::High)
            .opacity(card_opacity)
            .owns_clicks()
            .child(webview);
        // The buttons ride on the card's top-right corner, on a raised pill
        // that reads over any page.
        let controls_height = metrics::peek_controls_height();
        let buttons = h_stack()
            .absolute()
            .top(card_bounds.origin.y - px(controls_height + metrics::PEEK_CONTROL_GAP))
            .right(window.viewport_size().width - card_bounds.right())
            .h(px(controls_height))
            .p(px(metrics::MENU_PADDING))
            .gap(px(metrics::PEEK_CONTROL_GAP))
            .rounded_full()
            .border_1()
            .border_color(rgba(palette.menu_border))
            .raised(palette)
            .opacity(shown)
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

        let mut overlay = div()
            .id("peek")
            .absolute()
            .inset_0()
            .child(backdrop)
            .child(card)
            .child(buttons);
        // A closing preview no longer holds the pointer or keys.
        if open {
            overlay = overlay
                .occlude()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| this.close_peek(window, cx)),
                )
                .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    if event.keystroke.key == "escape" && !event.keystroke.modifiers.modified() {
                        cx.stop_propagation();
                        this.close_peek(window, cx);
                    }
                }));
        }
        Some(overlay.into_any_element())
    }
}

/// How far the preview sits in from the window's sides and top, in whole
/// pixels: the page is laid out at a whole size, and a card at a fractional
/// one would shift it by a fraction of a pixel when it stops scaling.
fn peek_insets(window: &Window) -> (f32, f32) {
    let viewport = window.viewport_size();
    (
        (f32::from(viewport.width) * metrics::PEEK_INSET_FRACTION).round(),
        (f32::from(viewport.height) * metrics::PEEK_INSET_FRACTION / 2.0).round(),
    )
}

/// Where the card lands, under its buttons.
fn peek_frame(window: &Window) -> Bounds<Pixels> {
    let viewport = window.viewport_size();
    let (inset_x, inset_y) = peek_insets(window);
    let card_top = inset_y + metrics::peek_controls_height() + metrics::PEEK_CONTROL_GAP;
    bounds(
        point(px(inset_x), px(card_top)),
        size(
            viewport.width - px(inset_x * 2.0),
            viewport.height - px(inset_y + card_top),
        ),
    )
}

/// The page's size in a card at `frame`, inside the card's border, so the
/// page loaded ahead of time needs no resize when the card lands.
fn peek_page_size(frame: Bounds<Pixels>) -> gpui::Size<Pixels> {
    let border = px(metrics::SURFACE_BORDER_WIDTH * 2.0);
    size(frame.size.width - border, frame.size.height - border)
}

/// The card's bounds `progress` of the way from a link-sized copy of `frame`
/// around `origin` to `frame`. Every step keeps the frame's shape, so the page
/// scaled into it is never stretched.
fn morph_bounds(origin: Point<Pixels>, frame: Bounds<Pixels>, progress: f32) -> Bounds<Pixels> {
    let start_width = metrics::PEEK_MORPH_ORIGIN_WIDTH;
    let start_height =
        start_width * f32::from(frame.size.height) / f32::from(frame.size.width).max(1.0);
    let start_left = f32::from(origin.x) - start_width / 2.0;
    let start_top = f32::from(origin.y) - start_height / 2.0;
    bounds(
        point(
            px(mix(start_left, f32::from(frame.origin.x), progress)),
            px(mix(start_top, f32::from(frame.origin.y), progress)),
        ),
        size(
            px(mix(start_width, f32::from(frame.size.width), progress)),
            px(mix(start_height, f32::from(frame.size.height), progress)),
        ),
    )
}
