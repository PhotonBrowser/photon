//! Two tabs side by side, or an empty trailing pane waiting for a tab choice.
//! A click in a page makes its tab active; the divider sets
//! how the panes share the width. Panes slide in as the split opens and out
//! as one closes, and a tab dragged over the page shows where it would split.

use gpui::{
    AnyElement, App, Context, DragMoveEvent, Empty, ImageSource, ObjectFit, Pixels, Point, Render,
    SharedString, Size, Window, div, img, prelude::*, px, relative, rgba,
};
use photon_core::site_name;
use std::time::Instant;

use super::super::icons::{globe_icon, photon_logo, sleeping_icon};
use super::super::metrics;
use super::super::motion::{AnimateIn, Entrance, Speed, Tween, TweenCurve};
use super::super::pages::{PageIcon, find_page};
use super::super::settings::Settings;
use super::super::theme::ThemeColors;
use super::super::{ClickHandler, Favicon};
use super::BrowserWindow;
use super::content::{BrowserTab, TabContent};
use super::tab_summary::tab_summary;

mod picker;

/// One side of a split.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SplitSide {
    Leading,
    Trailing,
}

impl SplitSide {
    fn other(self) -> Self {
        match self {
            Self::Leading => Self::Trailing,
            Self::Trailing => Self::Leading,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Leading => "leading",
            Self::Trailing => "trailing",
        }
    }
}

/// The tabs shown in a split: a leading page and an optional trailing choice.
pub(super) struct SplitView {
    leading: u64,
    trailing: Option<u64>,
    /// The leading pane's share of the width once the split has opened.
    ratio: f32,
    /// How far the moving pane has come in: 0 gone, 1 in place.
    presence: Tween,
    /// The pane that slides in as the split opens, or out as it closes.
    moving: SplitSide,
    /// Whether the split ends once the moving pane has gone.
    closing: bool,
}

struct SplitChoice {
    id: u64,
    title: SharedString,
    address: SharedString,
    icon: AnyElement,
    on_select: ClickHandler,
}

impl SplitView {
    /// The leading pane's share of the width at `presence`.
    fn leading_share(&self, presence: f32) -> f32 {
        match self.moving {
            SplitSide::Trailing => 1.0 - presence * (1.0 - self.ratio),
            SplitSide::Leading => presence * self.ratio,
        }
    }

    fn id(&self, side: SplitSide) -> Option<u64> {
        match side {
            SplitSide::Leading => Some(self.leading),
            SplitSide::Trailing => self.trailing,
        }
    }

    fn side_of(&self, id: u64) -> Option<SplitSide> {
        if id == self.leading {
            Some(SplitSide::Leading)
        } else if self.trailing == Some(id) {
            Some(SplitSide::Trailing)
        } else {
            None
        }
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

/// How the drop indicator and the chooser's cards come in.
const DROP_INDICATOR_ENTRANCE: Entrance = Entrance::new()
    .fade_from(0.0)
    .scale_from(0.97)
    .speed(Speed::Quick);

impl BrowserWindow {
    /// The tab shown beside the tab at `index`, when the two are split.
    pub(super) fn split_partner_of(&self, index: usize) -> Option<usize> {
        let split = self.split_view.as_ref()?;
        let id = self.tabs.get(index)?.id;
        let partner = split.id(split.side_of(id)?.other())?;
        self.tabs.iter().position(|tab| tab.id == partner)
    }

    /// The tab grouped with the tab at `index` in a split that is not
    /// closing, whether or not either is active.
    pub(super) fn split_group_partner(&self, index: usize) -> Option<usize> {
        self.split_view
            .as_ref()
            .filter(|split| !split.closing)
            .and(self.split_partner_of(index))
    }

    /// The tab shown beside the active one, while a split shows.
    pub(super) fn split_partner(&self) -> Option<usize> {
        self.split_partner_of(self.active_tab?)
    }

    /// Whether the tab at `index` is on screen: active, or beside it.
    pub(super) fn shows_tab(&self, index: usize) -> bool {
        self.active_tab == Some(index) || self.split_partner() == Some(index)
    }

    fn active_id(&self) -> Option<u64> {
        self.active_tab
            .and_then(|index| self.tabs.get(index))
            .map(|tab| tab.id)
    }

    /// Whether the split is visible and is not on its way out.
    pub(super) fn split_open(&self) -> bool {
        let (Some(split), Some(active)) = (self.split_view.as_ref(), self.active_id()) else {
            return false;
        };
        !split.closing && split.side_of(active).is_some()
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
        if let Some(active) = self.active_id() {
            self.set_split_view(active, None, SplitSide::Trailing, window, cx);
        }
    }

    /// Makes the tab at `index` active and opens an empty pane beside it,
    /// whose chooser and address field pick what it shows.
    pub(super) fn create_split_view(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.tabs.get(index).map(|tab| tab.id) else {
            return;
        };
        if self.split_open() {
            self.close_split_view(false, cx);
        }
        if self.active_tab != Some(index) {
            self.activate_tab(index, false, window, cx);
        }
        self.set_split_view(id, None, SplitSide::Trailing, window, cx);
    }

    /// Shows the tab at `index` beside the active one, filling the empty
    /// pane when one waits for a choice.
    pub(super) fn split_with_tab(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (Some(active), Some(other)) = (self.active_id(), self.tabs.get(index).map(|t| t.id))
        else {
            return;
        };
        if active == other || (self.split_open() && self.split_partner() == Some(index)) {
            return;
        }
        if self.split_selection_pending()
            && let Some(split) = self.split_view.as_mut()
        {
            split.trailing = Some(other);
            self.activate_tab(index, true, window, cx);
            return;
        }
        self.set_split_view(active, Some(other), SplitSide::Trailing, window, cx);
    }

    fn set_split_view(
        &mut self,
        leading: u64,
        trailing: Option<u64>,
        moving: SplitSide,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_split_view(false, cx);
        let mut presence = Tween::at_with_curve(0.0, TweenCurve::EaseOut);
        presence.animate_to(1.0, Speed::Standard, &Settings::motion(cx));
        let pending = trailing.is_none();
        self.split_view = Some(SplitView {
            leading,
            trailing,
            ratio: metrics::SPLIT_VIEW_OPENING_SHARE,
            presence,
            moving,
            closing: false,
        });
        if pending {
            // The empty pane is ready for an address straight away: the
            // sidebar's field opens what is entered there in the new pane.
            self.omnibox
                .update(cx, |omnibox, cx| omnibox.focus(window, cx));
            cx.notify();
        } else {
            self.show_split_partner(window, cx);
        }
    }

    /// Puts the split's tabs back to one at a time, keeping the active tab;
    /// `animated`, the other pane slides out first.
    pub(super) fn close_split_view(&mut self, animated: bool, cx: &mut Context<Self>) {
        let animations = Settings::motion(cx);
        let active = self.active_id();
        if animated
            && animations.allows_motion()
            && self.split_open()
            && let Some(split) = self.split_view.as_mut()
        {
            if let Some(side) = active.and_then(|id| split.side_of(id)) {
                split.moving = side.other();
            }
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
        let active_id = self.active_id();
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
        let active_is_split = self
            .active_id()
            .is_some_and(|id| split.side_of(id).is_some());
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

    /// Opens `url` in a new tab beside the leading one and shows it in the
    /// empty pane.
    pub(super) fn fill_split_pane(
        &mut self,
        url: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(leading) = self.active_tab else {
            return;
        };
        // A page only one tab may show fills the pane from its open tab.
        let open = find_page(url).and_then(|page| self.single_page_tab(page));
        // Already shown beside it, it is not opened a second time.
        if let Some(index) = open {
            if index != leading {
                self.split_with_tab(index, window, cx);
            }
            return;
        }
        let Some(id) = self.insert_address(leading + 1, url, false, window, cx) else {
            return;
        };
        if let Some(split) = self.split_view.as_mut() {
            split.trailing = Some(id);
        }
        let Some(index) = self.tab_index_by_id(id) else {
            return;
        };
        self.activate_tab(index, true, window, cx);
    }

    /// Where a dropped tab's pane meets the other: an open split's divider,
    /// or where a new split would divide the page. Both where a drop lands
    /// and where its indicator is drawn follow it.
    fn split_drop_boundary(&self) -> f32 {
        self.split_view
            .as_ref()
            .filter(|split| self.split_open() && split.trailing.is_some())
            .map_or(metrics::SPLIT_VIEW_OPENING_SHARE, |split| split.ratio)
    }

    /// The side of the page a tab dragged to `position` would split into,
    /// if it can split there.
    pub(super) fn split_drop_side(&self, id: u64, position: Point<Pixels>) -> Option<SplitSide> {
        let bounds = self.page_bounds?;
        if !bounds.contains(&position) || self.tab_index_by_id(id).is_none() {
            return None;
        }
        // A sidebar revealed over the page takes what is dropped on it.
        let over_sidebar = self.sidebar.over_page()
            && self
                .sidebar_panel_bounds
                .is_some_and(|panel| panel.contains(&position));
        if over_sidebar {
            return None;
        }
        let active = self.active_id()?;
        let open = self.split_open() && !self.split_selection_pending();
        if !open && id == active {
            return None;
        }
        let width = f32::from(bounds.size.width);
        if width <= 0.0 {
            return None;
        }
        let x = f32::from(position.x - bounds.origin.x) / width;
        let boundary = self.split_drop_boundary();
        let side = if open {
            if x < boundary {
                SplitSide::Leading
            } else {
                SplitSide::Trailing
            }
        } else if x < metrics::SPLIT_DROP_EDGE_SHARE {
            SplitSide::Leading
        } else if x > 1.0 - metrics::SPLIT_DROP_EDGE_SHARE {
            SplitSide::Trailing
        } else {
            // The page's middle is not a split target.
            return None;
        };
        // A pane already showing the tab has nothing to change.
        let unchanged = self
            .split_view
            .as_ref()
            .filter(|_| open)
            .is_some_and(|split| split.id(side) == Some(id));
        (!unchanged).then_some(side)
    }

    /// Shows the dragged tab on `side` of the page: beside the active tab,
    /// in place of a pane's tab, or swapped with its partner.
    pub(super) fn drop_tab_in_split(
        &mut self,
        id: u64,
        side: SplitSide,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(active) = self.active_id() else {
            return;
        };
        // A pane waiting for a tab takes the dropped one in place, as picking
        // it from the chooser does, without the split opening again.
        if self.split_selection_pending() {
            if let Some(split) = self.split_view.as_mut()
                && split.leading != id
            {
                match side {
                    SplitSide::Trailing => split.trailing = Some(id),
                    SplitSide::Leading => {
                        split.trailing = Some(split.leading);
                        split.leading = id;
                    }
                }
            }
            if let Some(index) = self.tab_index_by_id(id) {
                self.activate_tab(index, true, window, cx);
            }
            cx.notify();
            return;
        }
        let open = self.split_open();
        let replaced = self
            .split_view
            .as_ref()
            .and_then(|split| split.id(side))
            .and_then(|old| self.tab_index_by_id(old));
        if open && let Some(split) = self.split_view.as_mut() {
            match split.side_of(id) {
                Some(current) if current == side => {}
                Some(_) => {
                    if let Some(trailing) = split.trailing {
                        split.trailing = Some(split.leading);
                        split.leading = trailing;
                        split.ratio = 1.0 - split.ratio;
                    }
                }
                None => {
                    match side {
                        SplitSide::Leading => split.leading = id,
                        SplitSide::Trailing => split.trailing = Some(id),
                    }
                    if let Some(old) = replaced
                        && self.active_tab != Some(old)
                    {
                        self.hide_tab_page(old, cx);
                    }
                }
            }
        } else if id != active {
            match side {
                SplitSide::Trailing => {
                    self.set_split_view(active, Some(id), SplitSide::Trailing, window, cx)
                }
                SplitSide::Leading => {
                    self.set_split_view(id, Some(active), SplitSide::Leading, window, cx)
                }
            }
        }
        if let Some(index) = self.tab_index_by_id(id) {
            self.activate_tab(index, true, window, cx);
        }
        cx.notify();
    }

    /// Marks where a tab dragged over the page would go.
    pub(super) fn split_drop_indicator(
        &self,
        palette: ThemeColors,
        cx: &App,
    ) -> Option<AnyElement> {
        let side = self.split_drop?;
        // Drawn only while the dragged tab is still being dragged, so a
        // drag ended any other way, as by closing its tab, leaves no mark.
        self.tab_drag_layout
            .as_ref()
            .filter(|drag| self.tab_index_by_id(drag.id).is_some())?;
        let boundary = self.split_drop_boundary();
        let (start, width) = match side {
            SplitSide::Leading => (0.0, boundary),
            SplitSide::Trailing => (boundary, 1.0 - boundary),
        };
        let inset = px(metrics::SPLIT_DROP_INDICATOR_INSET);
        let indicator = div()
            .size_full()
            .rounded(px(metrics::page_radius()))
            .border(px(metrics::SPLIT_DROP_INDICATOR_BORDER))
            .border_color(rgba(palette.soft_accent_border))
            .bg(rgba(palette.selected_surface));
        Some(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(relative(start))
                .w(relative(width))
                .p(inset)
                .child(indicator.animate_in(
                    SharedString::from(format!("split-drop-{}", side.name())),
                    DROP_INDICATOR_ENTRANCE,
                    &Settings::motion(cx),
                ))
                .into_any_element(),
        )
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
        let moving_side = split.moving;
        let ratio = split.ratio;
        let share = split.leading_share(presence);
        if moving {
            window.request_animation_frame();
        }

        // While it slides, the moving page keeps its open layout, clipped
        // to where it is drawn, rather than laying out again every frame.
        let slide_growth = match moving_side {
            SplitSide::Trailing => share - ratio,
            SplitSide::Leading => ratio - share,
        } * width;
        let moving_index = match moving_side {
            SplitSide::Leading => Some(leading),
            SplitSide::Trailing => trailing,
        };
        if let Some(index) = moving_index
            && let Some(webview) = self.tabs[index].content.webview()
        {
            let is_active = index == active;
            let growth = Size {
                width: slide_growth + if is_active { layout_growth.width } else { 0.0 },
                height: if is_active { layout_growth.height } else { 0.0 },
            };
            webview.update(cx, |view, _| {
                view.set_layout_growth(growth);
                view.set_page_opacity(presence);
            });
        }

        let half_gap = px(metrics::PAGE_INSET / 2.0 * presence);
        let pane_opacity = |side: SplitSide| if side == moving_side { presence } else { 1.0 };
        let leading_pane = self
            .split_pane(
                Some(leading),
                active,
                pane_opacity(SplitSide::Leading),
                palette,
                cx,
            )
            .flex_none()
            .w(relative(share))
            .pr(half_gap);
        let trailing_pane = self
            .split_pane(
                trailing,
                active,
                pane_opacity(SplitSide::Trailing),
                palette,
                cx,
            )
            .flex_1()
            .pl(half_gap);
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

    /// One pane: its tab's page, or the chooser while it waits for a tab.
    fn split_pane(
        &mut self,
        index: Option<usize>,
        active: usize,
        opacity: f32,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let page = match index {
            Some(index) => self.tab_view(Some(index), palette, cx),
            None => {
                let leading_id = self.tabs[active].id;
                let choices = self.split_choices(leading_id, palette, cx);
                // The page's native layer fades itself; only the chooser,
                // drawn by GPUI, takes the pane's opacity.
                div()
                    .size_full()
                    .opacity(opacity)
                    .child(picker::view(choices, palette, &Settings::motion(cx)))
                    .into_any_element()
            }
        };
        div()
            .relative()
            .h_full()
            .min_w_0()
            .overflow_hidden()
            .when_some(index, |pane, index| {
                pane.capture_any_mouse_down(cx.listener(move |this, _, window, cx| {
                    if this.active_tab != Some(index) {
                        this.activate_tab(index, true, window, cx);
                    }
                }))
            })
            .child(page)
    }

    fn split_choices(
        &self,
        leading_id: u64,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> Vec<SplitChoice> {
        // This space's tabs, and favourites' open tabs, which join whichever
        // space they are shown in.
        let space = self.shows_spaces(cx).then(|| self.current_space(cx));
        self.tabs
            .iter()
            .filter(|tab| {
                tab.id != leading_id
                    && space.is_none_or(|space| tab.space == space || tab.favourite.is_some())
            })
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
                    if let Some(index) = this.tab_index_by_id(id) {
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
    use super::{SplitSide, metrics::SPLIT_VIEW_MIN_SHARE, split_ratio};

    #[test]
    fn panes_keep_a_usable_width() {
        assert_eq!(split_ratio(0.5), 0.5);
        assert_eq!(split_ratio(0.0), SPLIT_VIEW_MIN_SHARE);
        assert_eq!(split_ratio(1.0), 1.0 - SPLIT_VIEW_MIN_SHARE);
    }

    #[test]
    fn sides_are_opposite() {
        assert_eq!(SplitSide::Leading.other(), SplitSide::Trailing);
        assert_eq!(SplitSide::Trailing.other(), SplitSide::Leading);
    }
}
