//! Two tabs side by side. The split shows while either of its tabs is
//! active; a click in a pane makes that pane's tab active, and the divider
//! between them sets how they share the width. The second pane slides and
//! fades in when the split opens, and back out when it closes.

use gpui::{
    AnyElement, Context, DragMoveEvent, Empty, Render, Size, Window, div, prelude::*, px, relative,
};
use std::time::Instant;

use super::super::metrics;
use super::super::motion::{Speed, Tween, TweenCurve};
use super::super::settings::Settings;
use super::super::theme::ThemeColors;
use super::BrowserWindow;

/// The pair of tabs shown together, by tab ID, leading first.
pub(super) struct SplitView {
    tabs: [u64; 2],
    /// The leading pane's share of the width once the split has opened.
    ratio: f32,
    /// How far the trailing pane has come in: 0 closed, 1 open.
    presence: Tween,
    /// Whether the split ends once the trailing pane has gone.
    closing: bool,
}

impl SplitView {
    /// The leading pane's share of the width at `presence`.
    fn leading_share(&self, presence: f32) -> f32 {
        1.0 - presence * (1.0 - self.ratio)
    }
}

/// What the divider drags.
#[derive(Clone)]
pub(super) struct SplitDividerDrag;

impl Render for SplitDividerDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

impl BrowserWindow {
    /// The tab shown beside the tab at `index`, when the two are split.
    pub(super) fn split_partner_of(&self, index: usize) -> Option<usize> {
        let [leading, trailing] = self.split_view.as_ref()?.tabs;
        let id = self.tabs.get(index)?.id;
        let partner = match id {
            id if id == leading => trailing,
            id if id == trailing => leading,
            _ => return None,
        };
        self.tabs.iter().position(|tab| tab.id == partner)
    }

    /// The tab shown beside the active one, while a split shows.
    pub(super) fn split_partner(&self) -> Option<usize> {
        self.split_partner_of(self.active_tab?)
    }

    /// Whether the tab at `index` is on screen: active, or beside it.
    pub(super) fn shows_tab(&self, index: usize) -> bool {
        self.active_tab == Some(index) || self.split_partner() == Some(index)
    }

    /// Whether a split shows and is not on its way out.
    pub(super) fn split_open(&self) -> bool {
        self.split_partner().is_some() && self.split_view.as_ref().is_some_and(|s| !s.closing)
    }

    /// Ends the split showing, or splits the active tab with a new one.
    pub(super) fn toggle_split_view(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.split_open() {
            self.close_split_view(true, cx);
            return;
        }
        let Some(active) = self.active_tab.map(|index| self.tabs[index].id) else {
            return;
        };
        self.open_tab(window, cx);
        if let Some(index) = self.active_tab {
            self.split(active, self.tabs[index].id, window, cx);
        }
    }

    /// Shows the tab at `index` beside the active one.
    pub(super) fn split_with_tab(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (Some(active), Some(other)) = (
            self.active_tab.map(|index| self.tabs[index].id),
            self.tabs.get(index).map(|tab| tab.id),
        ) else {
            return;
        };
        if active != other && !(self.split_open() && self.split_partner() == Some(index)) {
            self.split(active, other, window, cx);
        }
    }

    fn split(&mut self, leading: u64, trailing: u64, window: &mut Window, cx: &mut Context<Self>) {
        self.close_split_view(false, cx);
        let mut presence = Tween::at_with_curve(0.0, TweenCurve::EaseOut);
        presence.animate_to(1.0, Speed::Standard, &Settings::motion(cx));
        self.split_view = Some(SplitView {
            tabs: [leading, trailing],
            ratio: 0.5,
            presence,
            closing: false,
        });
        self.show_split_partner(window, cx);
    }

    /// Puts the split's tabs back to one at a time, keeping the active tab;
    /// `animated`, the other pane slides out first.
    pub(super) fn close_split_view(&mut self, animated: bool, cx: &mut Context<Self>) {
        let animations = Settings::motion(cx);
        if animated
            && animations.allows_motion()
            && self.split_partner().is_some()
            && let Some(split) = self.split_view.as_mut()
        {
            split.closing = true;
            split.presence.animate_to(0.0, Speed::Standard, &animations);
        } else {
            if let Some(partner) = self.split_partner() {
                self.hide_tab_page(partner, cx);
            }
            self.split_view = None;
        }
        cx.notify();
    }

    /// Wakes and shows the page beside the active tab, after activation.
    pub(super) fn show_split_partner(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(partner) = self.split_partner() else {
            return;
        };
        if !self.wake_tab(partner, window, cx) {
            return;
        }
        if let Some(webview) = self.tabs[partner].content.webview() {
            let visible = self.window_visible;
            webview.update(cx, |view, cx| {
                view.set_page_visible(visible);
                cx.notify();
            });
        }
        cx.notify();
    }

    /// Hides the page of a tab that leaves the screen.
    pub(super) fn hide_tab_page(&self, index: usize, cx: &mut Context<Self>) {
        if let Some(webview) = self.tabs.get(index).and_then(|tab| tab.content.webview()) {
            webview.update(cx, |view, cx| {
                view.set_page_visible(false);
                view.set_page_opacity(1.0);
                view.set_layout_growth(Size::default());
                view.session.set_focus(false);
                cx.notify();
            });
        }
    }

    /// Ends a split that has slid shut, or one of whose tabs closed or left
    /// the window, hiding whichever of its pages is not active.
    pub(super) fn settle_split_view(&mut self, cx: &mut Context<Self>) {
        let Some(split) = self.split_view.as_ref() else {
            return;
        };
        let present = split
            .tabs
            .map(|id| self.tabs.iter().position(|tab| tab.id == id));
        let shut = split.closing && !split.presence.is_running();
        // A closing split left for another tab has nothing left to slide.
        let left_behind = split.closing && self.split_partner().is_none();
        if present.iter().all(Option::is_some) && !shut && !left_behind {
            return;
        }
        self.split_view = None;
        for index in present.into_iter().flatten() {
            if self.active_tab != Some(index) {
                self.hide_tab_page(index, cx);
            }
        }
    }

    /// The two panes and the divider between them, while a split shows.
    /// `width` is the page area's, and `layout_growth` the active page's
    /// while the chrome around it moves.
    pub(super) fn split_view(
        &mut self,
        width: f32,
        layout_growth: Size<f32>,
        palette: ThemeColors,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let partner = self.split_partner()?;
        let active = self.active_tab?;
        let split = self.split_view.as_ref()?;
        let now = Instant::now();
        let presence = split.presence.value_at(now);
        let moving = split.presence.is_running_at(now);
        let ratio = split.ratio;
        let share = split.leading_share(presence);
        let (leading, trailing) = if self.tabs[active].id == split.tabs[0] {
            (active, partner)
        } else {
            (partner, active)
        };
        if moving {
            window.request_animation_frame();
        }

        // While it slides, the trailing page keeps its open layout, clipped
        // to where it is drawn, rather than laying out again every frame.
        let slide_growth = (share - ratio) * width;
        let trailing_growth = Size {
            width: slide_growth
                + if trailing == active {
                    layout_growth.width
                } else {
                    0.0
                },
            height: if trailing == active {
                layout_growth.height
            } else {
                0.0
            },
        };
        if let Some(webview) = self.tabs[trailing].content.webview() {
            webview.update(cx, |view, _| {
                view.set_layout_growth(trailing_growth);
                view.set_page_opacity(presence);
            });
        }

        let half_gap = px(metrics::PAGE_INSET / 2.0 * presence);
        let leading_pane = self
            .split_pane(leading, palette, cx)
            .flex_none()
            .w(relative(share))
            .pr(half_gap);
        let trailing_pane = self.split_pane(trailing, palette, cx).flex_1().pl(half_gap);
        let divider = (!moving).then(|| {
            div()
                .id("split-view-divider")
                .absolute()
                .top_0()
                .bottom_0()
                .left(relative(ratio))
                .ml(px(-metrics::SPLIT_VIEW_DIVIDER_WIDTH / 2.0))
                .w(px(metrics::SPLIT_VIEW_DIVIDER_WIDTH))
                .cursor_col_resize()
                .on_drag(SplitDividerDrag, |_, _, _, cx| cx.new(|_| SplitDividerDrag))
        });
        Some(
            div()
                .id("split-view")
                .relative()
                .size_full()
                .flex()
                .flex_row()
                .child(leading_pane)
                .child(trailing_pane)
                .children(divider)
                .on_drag_move(cx.listener(Self::drag_split_divider))
                .into_any_element(),
        )
    }

    /// One pane, which activates its tab when clicked.
    fn split_pane(
        &mut self,
        index: usize,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        div()
            .relative()
            .h_full()
            .min_w_0()
            .overflow_hidden()
            .capture_any_mouse_down(cx.listener(move |this, _, window, cx| {
                if this.active_tab != Some(index) {
                    this.activate_tab(index, true, window, cx);
                }
            }))
            .child(self.tab_view(Some(index), palette, cx))
    }

    fn drag_split_divider(
        &mut self,
        event: &DragMoveEvent<SplitDividerDrag>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(split) = self.split_view.as_mut() else {
            return;
        };
        let width = f32::from(event.bounds.size.width);
        if width <= 0.0 {
            return;
        }
        let ratio = split_ratio(f32::from(event.event.position.x - event.bounds.origin.x) / width);
        if ratio != split.ratio {
            split.ratio = ratio;
            cx.notify();
        }
    }
}

/// Keeps both panes wide enough to use.
fn split_ratio(ratio: f32) -> f32 {
    ratio.clamp(
        metrics::SPLIT_VIEW_MIN_SHARE,
        1.0 - metrics::SPLIT_VIEW_MIN_SHARE,
    )
}

#[cfg(test)]
mod tests {
    use super::{metrics::SPLIT_VIEW_MIN_SHARE, split_ratio};

    #[test]
    fn panes_keep_a_usable_width() {
        assert_eq!(split_ratio(0.5), 0.5);
        assert_eq!(split_ratio(0.0), SPLIT_VIEW_MIN_SHARE);
        assert_eq!(split_ratio(1.0), 1.0 - SPLIT_VIEW_MIN_SHARE);
    }
}
