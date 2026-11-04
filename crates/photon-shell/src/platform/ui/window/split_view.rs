//! Two tabs side by side, or an empty trailing pane waiting for a tab choice.
//! A click in a page makes its tab active; the divider sets how the panes
//! share the width. The trailing pane slides in when the split opens.

use gpui::{
    AnyElement, App, Context, DragMoveEvent, Empty, ImageSource, ObjectFit, Render, SharedString,
    Size, Window, div, img, prelude::*, px, relative,
};
use photon_core::site_name;
use std::time::Instant;

use super::super::Favicon;
use super::super::icons::{globe_icon, photon_logo, sleeping_icon};
use super::super::metrics;
use super::super::motion::{Speed, Tween, TweenCurve};
use super::super::pages::PageIcon;
use super::super::settings::Settings;
use super::super::theme::ThemeColors;
use super::BrowserWindow;
use super::content::{BrowserTab, TabContent};
use super::tab_summary::tab_summary;

mod picker;

/// The tabs shown in a split: a leading page and an optional trailing choice.
pub(super) struct SplitView {
    leading: u64,
    trailing: Option<u64>,
    /// The leading pane's share of the width once the split has opened.
    ratio: f32,
    /// How far the trailing pane has come in: 0 closed, 1 open.
    presence: Tween,
    /// Whether the split ends once the trailing pane has gone.
    closing: bool,
}

struct SplitChoice {
    id: u64,
    title: SharedString,
    address: SharedString,
    icon: AnyElement,
    on_select: super::super::ClickHandler,
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
        let split = self.split_view.as_ref()?;
        let id = self.tabs.get(index)?.id;
        let partner = match id {
            id if id == split.leading => split.trailing?,
            id if split.trailing == Some(id) => split.leading,
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

    /// Whether the split is visible and is not on its way out.
    pub(super) fn split_open(&self) -> bool {
        let Some(split) = self.split_view.as_ref() else {
            return false;
        };
        let Some(active) = self.active_tab.and_then(|index| self.tabs.get(index)) else {
            return false;
        };
        !split.closing && (active.id == split.leading || split.trailing == Some(active.id))
    }

    /// Whether the split's trailing pane is waiting for a tab choice.
    pub(super) fn split_selection_pending(&self) -> bool {
        self.split_open()
            && self
                .split_view
                .as_ref()
                .is_some_and(|split| split.trailing.is_none())
    }

    /// Ends a split, or opens an empty pane beside the active tab.
    pub(super) fn toggle_split_view(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.split_open() {
            self.close_split_view(true, cx);
            return;
        }
        let Some(active) = self.active_tab.map(|index| self.tabs[index].id) else {
            return;
        };
        self.set_split_view(active, None, window, cx);
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
            self.set_split_view(active, Some(other), window, cx);
        }
    }

    fn set_split_view(
        &mut self,
        leading: u64,
        trailing: Option<u64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_split_view(false, cx);
        let mut presence = Tween::at_with_curve(0.0, TweenCurve::EaseOut);
        presence.animate_to(1.0, Speed::Standard, &Settings::motion(cx));
        self.split_view = Some(SplitView {
            leading,
            trailing,
            ratio: 0.5,
            presence,
            closing: false,
        });
        if trailing.is_some() {
            self.show_split_partner(window, cx);
        } else {
            cx.notify();
        }
    }

    /// Puts the split's tabs back to one at a time, keeping the active tab;
    /// `animated`, the other pane slides out first.
    pub(super) fn close_split_view(&mut self, animated: bool, cx: &mut Context<Self>) {
        let animations = Settings::motion(cx);
        if animated
            && animations.allows_motion()
            && self.split_open()
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

    /// Keeps the leading tab current while choosing, then wakes the split partner.
    pub(super) fn show_split_partner(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let active_id = self
            .active_tab
            .and_then(|index| self.tabs.get(index).map(|tab| tab.id));
        if let Some(split) = self
            .split_view
            .as_mut()
            .filter(|split| split.trailing.is_none())
        {
            if let Some(active_id) = active_id {
                split.leading = active_id;
            }
            return;
        }
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
        let leading = self.tabs.iter().position(|tab| tab.id == split.leading);
        let trailing_id = split.trailing;
        let trailing = trailing_id.and_then(|id| self.tabs.iter().position(|tab| tab.id == id));
        let shut = split.closing && !split.presence.is_running();
        let active_is_split = self.active_tab.is_some_and(|index| {
            self.tabs
                .get(index)
                .is_some_and(|tab| tab.id == split.leading || split.trailing == Some(tab.id))
        });
        // A closing split left for another tab has nothing left to slide.
        let left_behind = split.closing && !active_is_split;
        let missing_tab = leading.is_none() || (trailing_id.is_some() && trailing.is_none());
        if !missing_tab && !shut && !left_behind {
            return;
        }
        self.split_view = None;
        for index in [leading, trailing].into_iter().flatten() {
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
        let active = self.active_tab?;
        let split = self.split_view.as_ref()?;
        let leading_id = split.leading;
        let leading = self.tabs.iter().position(|tab| tab.id == leading_id)?;
        let trailing_id = split.trailing;
        let trailing = trailing_id.and_then(|id| self.tabs.iter().position(|tab| tab.id == id));
        if trailing_id.is_some() && trailing.is_none() {
            return None;
        }
        if active != leading && Some(active) != trailing {
            return None;
        }
        let now = Instant::now();
        let presence = split.presence.value_at(now);
        let moving = split.presence.is_running_at(now);
        let ratio = split.ratio;
        let share = split.leading_share(presence);
        if moving {
            window.request_animation_frame();
        }

        // While it slides, the trailing page keeps its open layout, clipped
        // to where it is drawn, rather than laying out again every frame.
        let slide_growth = (share - ratio) * width;
        let trailing_growth = Size {
            width: slide_growth
                + if Some(active) == trailing {
                    layout_growth.width
                } else {
                    0.0
                },
            height: if Some(active) == trailing {
                layout_growth.height
            } else {
                0.0
            },
        };
        if let Some(webview) = trailing.and_then(|index| self.tabs[index].content.webview()) {
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
        let trailing_pane = if let Some(trailing) = trailing {
            self.split_pane(trailing, palette, cx).flex_1().pl(half_gap)
        } else {
            let choices = self.split_choices(leading_id, palette, cx);
            div()
                .relative()
                .h_full()
                .min_w_0()
                .overflow_hidden()
                .flex_1()
                .pl(half_gap)
                .child(picker::view(choices, palette, presence))
        };
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

    fn split_choices(
        &self,
        leading_id: u64,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> Vec<SplitChoice> {
        self.tabs
            .iter()
            .filter(|tab| tab.id != leading_id)
            .map(|tab| {
                let summary = tab_summary(tab, cx);
                let title = if summary.title.trim().is_empty() {
                    "New Tab".to_owned()
                } else {
                    summary.title
                };
                let address = if summary.url.trim().is_empty() {
                    "New tab".to_owned()
                } else {
                    site_name(&summary.url)
                };
                let id = tab.id;
                let on_select = cx.listener(move |this, _, window, cx| {
                    if let Some(index) = this.tabs.iter().position(|tab| tab.id == id) {
                        this.split_with_tab(index, window, cx);
                    }
                });
                SplitChoice {
                    id,
                    title: title.into(),
                    address: address.into(),
                    icon: split_choice_icon(tab, palette, cx),
                    on_select: Box::new(on_select),
                }
            })
            .collect()
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

fn split_choice_icon(tab: &BrowserTab, palette: ThemeColors, cx: &App) -> AnyElement {
    match &tab.content {
        TabContent::Page(page) => match page.definition.icon {
            PageIcon::Logo => img(photon_logo(palette.logo_tint))
                .size(px(metrics::SPLIT_CHOOSER_ICON_SIZE))
                .object_fit(ObjectFit::Contain)
                .into_any_element(),
            PageIcon::Symbol(symbol) => {
                symbol(palette.text_primary, metrics::SPLIT_CHOOSER_ICON_SIZE)
            }
        },
        TabContent::Sleeping(tab) => tab
            .favicon
            .as_ref()
            .map(favicon_choice_icon)
            .unwrap_or_else(|| {
                sleeping_icon(palette.text_secondary, metrics::SPLIT_CHOOSER_ICON_SIZE)
                    .into_any_element()
            }),
        TabContent::Web(webview) => webview
            .read(cx)
            .favicon
            .as_ref()
            .map(favicon_choice_icon)
            .unwrap_or_else(|| {
                globe_icon(palette.text_secondary, metrics::SPLIT_CHOOSER_ICON_SIZE)
                    .into_any_element()
            }),
    }
}

fn favicon_choice_icon(favicon: &Favicon) -> AnyElement {
    img(ImageSource::Render(favicon.image.clone()))
        .size(px(metrics::SPLIT_CHOOSER_ICON_SIZE))
        .object_fit(ObjectFit::Contain)
        .into_any_element()
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
