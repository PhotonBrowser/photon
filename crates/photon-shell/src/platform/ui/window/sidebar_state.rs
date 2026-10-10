//! Where the sidebar is: shown beside the page or hidden, and while hidden,
//! revealed over the page by pointing at the window's left edge. Each moves
//! smoothly, and the page follows.

use gpui::{AnyElement, App, Context, Task, div, prelude::*, px};
use photon_core::TabLayout;
use std::time::Duration;

use super::super::metrics;
use super::super::motion::{Speed, Tween};
use super::super::settings::Settings;
use super::BrowserWindow;

/// How long the pointer may be away before the revealed sidebar leaves, so
/// crossing from the edge into the sidebar does not close it.
const REVEAL_GRACE: Duration = Duration::from_millis(150);

pub(super) struct SidebarState {
    /// Whether it is kept shown beside the page.
    pub(super) visible: bool,
    /// How far it is shown, from 0 to 1.
    shown: Tween,
    /// Whether it is revealed over the page while hidden.
    revealed: bool,
    /// How far it is revealed, from 0 to 1.
    reveal: Tween,
    /// Whether the pointer is over the left edge, and over the sidebar.
    pointer_on_edge: bool,
    pointer_on_sidebar: bool,
    _hide_check: Option<Task<()>>,
}

impl SidebarState {
    pub(super) fn new() -> Self {
        Self {
            visible: true,
            shown: Tween::at(1.0),
            revealed: false,
            reveal: Tween::at(0.0),
            pointer_on_edge: false,
            pointer_on_sidebar: false,
            _hide_check: None,
        }
    }

    /// How far it is shown beside the page, from 0 to 1.
    pub(super) fn shown(&self) -> f32 {
        self.shown.value()
    }

    /// How far it is revealed over the page, from 0 to 1.
    pub(super) fn revealed(&self) -> f32 {
        self.reveal.value()
    }

    /// Whether it sits over the page, revealed or on its way to being kept.
    pub(super) fn over_page(&self) -> bool {
        self.revealed || self.reveal.value() > 0.0
    }

    pub(super) fn is_moving(&self) -> bool {
        self.shown.is_running() || self.reveal.is_running()
    }

    /// Once a revealed sidebar has been kept shown and the page has made
    /// room for it, it is no longer revealed.
    pub(super) fn settle(&mut self) {
        if self.visible && self.revealed && !self.shown.is_running() {
            self.revealed = false;
            self.reveal.jump_to(0.0);
        }
    }

    fn set_visible(&mut self, visible: bool, cx: &App) -> bool {
        if self.visible == visible {
            return false;
        }
        self.visible = visible;
        self.shown
            .animate_to(if visible { 1.0 } else { 0.0 }, Speed::Gentle, cx);
        // Kept shown from a reveal, it stays where it is while the page
        // makes room; hidden, it leaves with the page.
        if !visible {
            self.revealed = false;
            self.reveal.jump_to(0.0);
        }
        true
    }

    fn set_revealed(&mut self, revealed: bool, cx: &App) -> bool {
        if self.revealed == revealed || (revealed && self.visible) {
            return false;
        }
        self.revealed = revealed;
        self.reveal
            .animate_to(if revealed { 1.0 } else { 0.0 }, Speed::Standard, cx);
        true
    }
}

impl BrowserWindow {
    /// Shows or hides the sidebar. The horizontal layout has none to hide.
    pub(super) fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        if Settings::get(cx).tab_layout == TabLayout::Vertical {
            self.set_sidebar_visible(!self.sidebar.visible, cx);
        }
    }

    pub(super) fn set_sidebar_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        if self.sidebar.set_visible(visible, cx) {
            self.open_menu = None;
            cx.notify();
        }
    }

    /// The window's left edge below the top bar, which reveals the sidebar.
    /// It lies in the margin beside the page, so it never covers the page.
    pub(super) fn reveal_edge(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("reveal-sidebar")
            .absolute()
            .top(px(metrics::TITLEBAR_HEIGHT))
            .bottom_0()
            .left_0()
            .w(px(metrics::PAGE_INSET))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.sidebar.pointer_on_edge = *hovered;
                if !*hovered {
                    this.hide_reveal_when_left(cx);
                } else if this.sidebar.set_revealed(true, cx) {
                    cx.notify();
                }
            }))
            .into_any_element()
    }

    /// Follows the pointer over the revealed sidebar.
    pub(super) fn revealed_sidebar_hovered(&mut self, hovered: bool, cx: &mut Context<Self>) {
        self.sidebar.pointer_on_sidebar = hovered;
        if !hovered {
            self.hide_reveal_when_left(cx);
        }
    }

    /// Hides the revealed sidebar shortly, unless the pointer comes back or a
    /// menu opened from it is still open.
    fn hide_reveal_when_left(&mut self, cx: &mut Context<Self>) {
        self.sidebar._hide_check = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(REVEAL_GRACE).await;
            this.update(cx, |this, cx| {
                let sidebar = &this.sidebar;
                if !sidebar.pointer_on_edge
                    && !sidebar.pointer_on_sidebar
                    && this.open_menu.is_none()
                    && this.sidebar.set_revealed(false, cx)
                {
                    cx.notify();
                }
            })
            .ok();
        }));
    }
}
