//! Opening, closing and switching tabs, and keeping the loading spinner
//! turning. `drag` follows tabs as they are dragged, reordered, dropped
//! on favourites or carried to another window.

use gpui::{App, Context, KeyDownEvent, Window};
use photon_core::Shortcut;
use photon_core::WindowSession;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use super::super::icons::LOADING_SPINNER_STEPS;
use super::super::motion::{Speed, Tween, TweenCurve};
use super::super::pages::{NEW_TAB, PageDefinition, find_page};
use super::super::settings::Settings;
use super::super::tabs::{ICON_ENTRANCE, TabDragAxis, TabIcon};
use super::BrowserWindow;
use super::content::{
    BrowserTab, TabContent, create_webview, create_webview_from_session, follow_appearance,
};
use crate::platform::app_update::update_app;
use crate::platform::engine::RequestedWebView;

mod drag;

/// A closed tab's page and position, for reopening.
pub(super) struct ClosedTab {
    index: usize,
    url: String,
}

/// How many closed tabs a window remembers.
const CLOSED_TAB_LIMIT: usize = 25;
static NEXT_TAB_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy)]
struct TabInsertionOptions {
    activate: bool,
    animate_appearance: bool,
}

impl BrowserWindow {
    pub(super) fn tab_axis(&self, cx: &App) -> TabDragAxis {
        if self.layout(cx).saved().tab_layout() == photon_core::TabLayout::Vertical {
            TabDragAxis::Vertical
        } else {
            TabDragAxis::Horizontal
        }
    }

    pub(super) fn tab_drag_active(&self) -> bool {
        self.incoming_tab_drag.is_some()
            || self.tab_drag_layout.as_ref().is_some_and(|drag| {
                !matches!(
                    drag.phase,
                    super::TabDragPhase::PendingDrag
                        | super::TabDragPhase::Completing
                        | super::TabDragPhase::Cancelled
                )
            })
    }

    pub(super) fn activate_tab(
        &mut self,
        index: usize,
        focus_contents: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.tabs.len() {
            return;
        }
        if !self.wake_tab(index, window, cx) {
            return;
        }
        if self.active_tab != Some(index) {
            let animations = Settings::motion(cx);
            let mut transition = Tween::at_with_curve(0.0, TweenCurve::SmoothStep);
            transition.animate_to_for(1.0, animations.duration(Speed::Standard), &animations);
            self.page_transition = animations.allows_motion().then_some(transition);
            // The find bar searches one page; switching tabs ends its search.
            self.close_find_bar(false, window, cx);
            if let Some(previous_index) = self.active_tab {
                if let Some(previous) = self.tabs.get_mut(previous_index) {
                    previous.last_active = Instant::now();
                }
                if let Some(previous) = self
                    .tabs
                    .get(previous_index)
                    .and_then(|tab| tab.content.webview())
                {
                    previous.update(cx, |view, cx| {
                        view.reset_history_swipe(true);
                        view.set_page_visible(false);
                        view.session.set_focus(false);
                        cx.notify();
                    });
                }
            }
            self.active_tab = Some(index);
        }
        self.tabs[index].last_active = Instant::now();
        // A tab of another space brings its space; a favourite's tab
        // belongs to whichever space it is opened in.
        let current = self.current_space(cx);
        if self.tabs[index].favourite.is_some() {
            self.tabs[index].space = current;
        } else if self.shows_spaces(cx) && self.tabs[index].space != current {
            self.switch_space(self.tabs[index].space, cx);
        }
        let focus_internal_page = focus_contents
            && matches!(
                &self.tabs[index].content,
                TabContent::Page(page) if !page.definition.is(NEW_TAB)
            );
        let webview = self.tabs[index].content.webview();
        if let Some(webview) = webview.as_ref() {
            let window_visible = self.window_visible;
            webview.update(cx, |view, cx| {
                view.set_page_visible(window_visible);
                // Prepaint places and shows the page's direct layer again.
                cx.notify();
                if focus_contents {
                    window.focus(&view.focus_handle, cx);
                }
                view.track_engine_focus(window, cx);
            });
        }
        self.omnibox.update(cx, |omnibox, cx| {
            omnibox.set_content(webview, self.tabs[index].content.page_address(), window, cx);
            if focus_contents
                && matches!(
                    &self.tabs[index].content,
                    TabContent::Page(page) if page.definition.is(NEW_TAB)
                )
            {
                omnibox.focus(window, cx);
            }
        });
        if focus_internal_page {
            window.focus(&self.root_focus, cx);
        }
        self.sync_dialog(window, cx);
        cx.notify();
        self.wake_tab_sleep_scheduler();
    }

    fn move_tab_focus(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.activate_tab(index, false, window, cx);
        window.focus(&self.tab_focus_handles[index], cx);
    }

    /// The new tab's position: at the top of today's tabs in a vertical
    /// sidebar, and at the end of the horizontal strip.
    pub(super) fn new_tab_index(&self, cx: &App) -> usize {
        if self.tab_axis(cx) != TabDragAxis::Vertical {
            return self.tabs.len();
        }

        match Settings::get(cx).new_tab_position {
            photon_core::NewTabPosition::Top => {
                let space = self.shows_spaces(cx).then(|| self.current_space(cx));
                self.tabs
                    .iter()
                    .position(|tab| {
                        tab.pin.is_none() && space.is_none_or(|space| tab.space == space)
                    })
                    .unwrap_or_else(|| self.pinned_count())
            }
            photon_core::NewTabPosition::AfterLast => self
                .listed_tabs(cx)
                .last()
                .map_or_else(|| self.pinned_count(), |index| index + 1),
        }
    }

    /// Gives vertical insertions one shared positional transition. At either
    /// edge, every moving row follows the same path so their spacing stays
    /// constant throughout the animation.
    fn prepare_vertical_tab_entry(
        &mut self,
        id: u64,
        index: usize,
        pinned: bool,
        animate_appearance: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        let section: Vec<_> = self
            .listed_tabs(cx)
            .into_iter()
            .filter(|&tab_index| self.tabs[tab_index].pin.is_some() == pinned)
            .collect();
        let first_index = section.first().copied();
        let last_index = section.last().copied();
        let at_top = first_index.is_none_or(|first| index <= first);
        let at_end = last_index.is_none_or(|last| index > last);
        let collapsed = self.sidebar.shows_icons();
        let spacing = super::super::metrics::SidebarSpacing::new(collapsed);
        let extent = if collapsed {
            super::super::metrics::SIDEBAR_COLLAPSED_ITEM_SIZE
        } else {
            super::super::metrics::SIDEBAR_TAB_HEIGHT
        };
        let shift = extent + spacing.item_gap;
        let settings = Settings::motion(cx);
        let animate = animate_appearance
            && settings.allows_motion()
            && !cx.reduce_motion()
            && (at_top || at_end);
        let duration = Duration::from_millis(super::super::metrics::TAB_INSERT_PUSH_DURATION_MS);

        if animate
            && at_end
            && !pinned
            && Settings::get(cx).new_tab_position == photon_core::NewTabPosition::AfterLast
        {
            let button_shift = extent
                + if last_index.is_some() {
                    spacing.item_gap
                } else {
                    0.0
                };
            let current_offset = self.new_tab_button_position_offset();
            self.start_new_tab_button_animation(current_offset - button_shift, duration, &settings);
        }

        for tab_index in section.into_iter().filter(|&tab_index| tab_index >= index) {
            let tab_id = self.tabs[tab_index].id;
            let current_offset = super::tab_position_animation_offset(
                self.tab_position_animations.get(&tab_id),
                TabDragAxis::Vertical,
            );
            let from = if animate && at_top {
                current_offset - shift
            } else {
                0.0
            };
            let animation = if animate && at_top {
                super::TabPositionAnimation::with_curve(
                    TabDragAxis::Vertical,
                    from,
                    duration,
                    &settings,
                    super::super::motion::TweenCurve::EaseOut,
                )
            } else {
                super::TabPositionAnimation::new(
                    TabDragAxis::Vertical,
                    0.0,
                    Duration::ZERO,
                    &settings,
                )
            };
            self.tab_position_animations.insert(tab_id, animation);
        }

        if animate {
            // At either edge, the new row enters from above and settles down
            // into its slot. The trailing button follows the same movement.
            let from = -shift;
            self.tab_position_animations.insert(
                id,
                super::TabPositionAnimation::with_curve(
                    TabDragAxis::Vertical,
                    from,
                    duration,
                    &settings,
                    super::super::motion::TweenCurve::EaseOut,
                ),
            );
        }

        animate
    }

    pub(super) fn open_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.insert_page(self.new_tab_index(cx), NEW_TAB, window, cx);
        let omnibox = self.omnibox.clone();
        cx.defer_in(window, move |_, window, cx| {
            omnibox.update(cx, |omnibox, cx| omnibox.focus(window, cx));
        });
    }

    pub(super) fn insert_tab(
        &mut self,
        index: usize,
        address: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(address) = address {
            if let Some(webview) = create_webview(cx, self.runtime.clone(), address) {
                self.insert_webview(index, webview, None, true, window, cx);
            }
        } else {
            self.insert_page(index, NEW_TAB, window, cx);
        }
    }

    /// Opens `address` in a new tab at the end: one of Photon's pages for a
    /// page's address, otherwise a web page.
    #[cfg_attr(feature = "ui-only", allow(dead_code))]
    pub(super) fn open_in_new_tab(
        &mut self,
        address: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<u64> {
        let previous_count = self.tabs.len();
        let existing_single_tab = find_page(address).is_some_and(|page| {
            page.single_tab
                && self.tabs.iter().any(|tab| {
                    matches!(&tab.content, TabContent::Page(shown) if shown.definition.is(page))
                })
        });
        match find_page(address) {
            Some(page) => self.open_page_tab(page, window, cx),
            None => self.insert_tab(self.new_tab_index(cx), Some(address), window, cx),
        }
        if self.tabs.len() > previous_count || existing_single_tab {
            self.active_tab.map(|index| self.tabs[index].id)
        } else {
            None
        }
    }

    /// Opens one of Photon's pages in a new tab, or switches to the tab
    /// already showing it when the page keeps to one tab.
    pub(super) fn open_page_tab(
        &mut self,
        page: &'static PageDefinition,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let open = page
            .single_tab
            .then(|| {
                self.tabs.iter().position(|tab| {
                    matches!(&tab.content, TabContent::Page(shown) if shown.definition.is(page))
                })
            })
            .flatten();
        match open {
            Some(index) => self.activate_tab(index, true, window, cx),
            None => self.insert_page(self.new_tab_index(cx), page, window, cx),
        }
    }

    /// Opens `address` in a new tab after `opener`'s, as a link opened from
    /// that page does.
    pub(super) fn open_tab_beside(
        &mut self,
        opener: &gpui::Entity<super::super::PhotonWebView>,
        address: &str,
        activate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        crate::platform::automation_lifecycle_trace(format_args!(
            "engine_new_tab_request tabs={} active_tab={:?} activate={} url={}",
            self.tabs.len(),
            self.active_tab,
            activate,
            address.split(['?', '#']).next().unwrap_or_default()
        ));
        let index = self
            .tabs
            .iter()
            .position(|tab| tab.content.webview().as_ref() == Some(opener))
            .map_or(self.tabs.len(), |index| index + 1);
        if let Some(webview) = create_webview(cx, self.runtime.clone(), address) {
            self.insert_webview(index, webview, None, activate, window, cx);
        }
    }

    pub(super) fn open_requested_tab(
        &mut self,
        request: RequestedWebView,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        crate::platform::automation_lifecycle_trace(format_args!(
            "engine_popup_request tabs={} active_tab={:?} activate={}",
            self.tabs.len(),
            self.active_tab,
            request.activate
        ));
        let webview = create_webview_from_session(cx, self.runtime.clone(), request.session);
        self.insert_webview(
            self.new_tab_index(cx),
            webview,
            None,
            request.activate,
            window,
            cx,
        );
    }

    /// Inserts a tab showing `webview`, pinned at `pin` or one of today's
    /// tabs. It adopts the window's page size so it lays out and loads even
    /// while in the background.
    pub(super) fn insert_webview(
        &mut self,
        index: usize,
        webview: gpui::Entity<super::super::PhotonWebView>,
        pin: Option<Shortcut>,
        activate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.insert_webview_with_appearance(
            index,
            webview,
            pin,
            TabInsertionOptions {
                activate,
                animate_appearance: true,
            },
            window,
            cx,
        );
    }

    #[cfg(not(feature = "ui-only"))]
    pub(super) fn insert_webview_immediately(
        &mut self,
        index: usize,
        webview: gpui::Entity<super::super::PhotonWebView>,
        pin: Option<Shortcut>,
        activate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.insert_webview_with_appearance(
            index,
            webview,
            pin,
            TabInsertionOptions {
                activate,
                animate_appearance: false,
            },
            window,
            cx,
        );
    }

    fn insert_webview_with_appearance(
        &mut self,
        index: usize,
        webview: gpui::Entity<super::super::PhotonWebView>,
        pin: Option<Shortcut>,
        options: TabInsertionOptions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let viewport = self
            .active_webview()
            .and_then(|webview| webview.read(cx).last_viewport);
        follow_appearance(&webview, window, cx);
        webview.update(cx, |view, cx| view.adopt_viewport(viewport, cx));
        let content = TabContent::Web(webview);
        match pin {
            Some(pin) => {
                self.insert_tab_at_with_appearance(index, content, Some(pin), options, window, cx)
            }
            None => {
                let index = index.clamp(self.pinned_count(), self.tabs.len());
                self.insert_tab_at_with_appearance(index, content, None, options, window, cx);
            }
        }
    }

    /// Inserts a tab showing one of Photon's own pages, and switches to it.
    pub(super) fn insert_page(
        &mut self,
        index: usize,
        page: &'static PageDefinition,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let content = self.new_page(page, cx);
        self.insert_content(index, content, true, window, cx);
    }

    /// Inserts a page without a row entrance, for when another container
    /// transition already brings the tab into view.
    pub(super) fn insert_page_immediately(
        &mut self,
        index: usize,
        page: &'static PageDefinition,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let content = self.new_page(page, cx);
        self.insert_tab_at_immediately(index, content, None, true, window, cx);
    }

    /// Inserts a tab of today's tabs, never among the pinned ones.
    pub(super) fn insert_content(
        &mut self,
        index: usize,
        content: TabContent,
        activate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let index = index.clamp(self.pinned_count(), self.tabs.len());
        self.insert_tab_at(index, content, None, activate, window, cx);
    }

    /// Inserts a tab, pinned at `pin` or one of today's tabs.
    pub(super) fn insert_tab_at(
        &mut self,
        index: usize,
        content: TabContent,
        pin: Option<Shortcut>,
        activate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.insert_tab_at_with_appearance(
            index,
            content,
            pin,
            TabInsertionOptions {
                activate,
                animate_appearance: true,
            },
            window,
            cx,
        );
    }

    pub(super) fn insert_tab_at_immediately(
        &mut self,
        index: usize,
        content: TabContent,
        pin: Option<Shortcut>,
        activate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.insert_tab_at_with_appearance(
            index,
            content,
            pin,
            TabInsertionOptions {
                activate,
                animate_appearance: false,
            },
            window,
            cx,
        );
    }

    fn insert_tab_at_with_appearance(
        &mut self,
        index: usize,
        content: TabContent,
        pin: Option<Shortcut>,
        options: TabInsertionOptions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let index = index.min(self.tabs.len());
        let page_address = content.page_address();
        let page = page_address.as_deref().unwrap_or_else(|| {
            if content.webview().is_some() {
                "webview"
            } else {
                "page"
            }
        });
        crate::platform::automation_lifecycle_trace(format_args!(
            "tab_insert index={} previous_count={} activate={} page={}",
            index,
            self.tabs.len(),
            options.activate,
            page.split(['?', '#']).next().unwrap_or_default()
        ));
        let subscription = self.subscribe_to_content(&content, window, cx);
        let id = Self::allocate_tab_id();
        let pinned = pin.is_some();
        if self.tab_axis(cx) == TabDragAxis::Vertical {
            if self.prepare_vertical_tab_entry(id, index, pinned, options.animate_appearance, cx) {
                self.tab_entry_animations.borrow_mut().insert(id);
            }
        } else if options.animate_appearance {
            self.tab_entry_animations.borrow_mut().insert(id);
        }
        let space = self.current_space(cx);
        self.tabs
            .insert(index, BrowserTab::new(id, content.clone(), pin, space));
        self.tab_subscriptions.insert(index, subscription);
        self.tab_focus_handles
            .insert(index, cx.focus_handle().tab_stop(self.tabs.len() == 1));
        if self
            .active_tab
            .is_some_and(|active_tab| index <= active_tab && self.tabs.len() > 1)
        {
            // Keep `active_tab` naming the same page until the switch below.
            self.active_tab = self.active_tab.map(|active_tab| active_tab + 1);
        }
        let activate = options.activate || (self.active_tab.is_none() && self.tabs.len() == 1);
        if activate {
            self.empty_page = None;
            self._empty_page_subscription = None;
            self.activate_tab(index, true, window, cx);
        } else {
            if let Some(webview) = content.webview() {
                webview.update(cx, |view, _| {
                    view.set_page_visible(false);
                    view.session.set_focus(false);
                });
            }
            cx.notify();
            self.wake_tab_sleep_scheduler();
        }
    }

    fn allocate_tab_id() -> u64 {
        NEXT_TAB_ID.fetch_add(1, Ordering::Relaxed)
    }

    pub(super) fn reopen_closed_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(closed) = self.closed_tabs.pop() {
            // Reopened unpinned, so never among the pinned tabs.
            let index = closed.index.clamp(self.pinned_count(), self.tabs.len());
            self.insert_tab(index, Some(&closed.url), window, cx);
        }
    }

    /// Activates the tab `offset` places from the active one, wrapping at the ends.
    pub(super) fn select_relative_tab(
        &mut self,
        offset: isize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(index) = self.listed_tab_after(self.active_tab.unwrap_or(0), offset, cx) {
            self.activate_tab(index, true, window, cx);
        }
    }

    pub(super) fn close_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }
        let id = self.tabs[index].id;
        let closes_last_window_tab = self.tabs.len() == 1 && Self::closes_with_last_tab(window, cx);
        crate::platform::automation_lifecycle_trace(format_args!(
            "tab_close_request tab_id={} index={} count={} active_tab={:?} closes_last_window_tab={}",
            id,
            index,
            self.tabs.len(),
            self.active_tab,
            closes_last_window_tab
        ));

        if self
            .tab_drag_layout
            .as_ref()
            .is_some_and(|drag| drag.id == id)
        {
            if let Some(drag) = self.tab_drag_layout.as_mut() {
                drag.phase = super::TabDragPhase::Cancelled;
            }
            self.clear_previous_target(cx);
            self.clear_favourites_drag_hover(window.window_handle(), id, cx);
            let _ = cx.stop_active_drag(window);
            window.release_pointer();
            self.tab_drag_layout = None;
            self.tab_position_animations.remove(&id);
            Self::notify_other_tab_drop_targets(cx, window.window_handle());
        }

        self.delete_tab(index, window, cx);
    }

    fn delete_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }
        let removed = self.tabs[index].clone();
        let closes_window = self.tabs.len() == 1 && Self::closes_with_last_tab(window, cx);
        if let Some(webview) = removed.content.webview() {
            let permission_view_id = webview.read(cx).session.permission_view_id();
            self.cancel_permissions_for_view(permission_view_id, window, cx);
            let favicon = webview.update(cx, |view, _| {
                view.set_page_visible(false);
                view.session.set_focus(false);
                view.favicon.take()
            });
            if let Some(favicon) = favicon {
                favicon.release(Some(window), cx);
            }
        }
        self.revealed_icons.borrow_mut().remove(&removed.id);
        self.tab_entry_animations.borrow_mut().remove(&removed.id);
        self.tab_position_animations.remove(&removed.id);
        self.tab_geometry.remove(&removed.id);
        self.wake_tab_sleep_scheduler();

        if closes_window {
            self.record_session_state(WindowSession::default(), cx);
            window.remove_window();
            return;
        }
        self.remember_closed(index, &removed.content, cx);

        let was_active = self.active_tab == Some(index);
        if was_active {
            // The bar searches this tab's page, so ends with it, and lets
            // the page go.
            self.close_find_bar(false, window, cx);
        }
        let next = was_active
            .then(|| self.tab_after_closing(index, cx))
            .flatten();
        let closed = self.tabs.remove(index);
        drop(self.tab_subscriptions.remove(index));
        self.tab_focus_handles.remove(index);
        if let Some(active_tab) = self.active_tab {
            if index < active_tab {
                self.active_tab = Some(active_tab - 1);
            } else if was_active {
                self.active_tab = next;
            }
        }

        if closed.pin.is_some() {
            self.forget_pin(&closed, cx);
        }
        if self.tabs.is_empty() {
            self.active_tab = None;
            self.close_find_bar(false, window, cx);
            self.omnibox.update(cx, |omnibox, cx| {
                omnibox.set_content(None, None, window, cx)
            });
            if self.tab_axis(cx) == TabDragAxis::Horizontal
                && !Self::closes_with_last_tab(window, cx)
            {
                self.open_tab(window, cx);
            } else if self.empty_page.is_none() {
                self.show_empty_new_tab_page(true, window, cx);
            }
            cx.notify();
            return;
        }
        if was_active {
            match self.active_tab {
                Some(active_tab) => self.activate_tab(active_tab, true, window, cx),
                // The space has no other tab; show it a new tab page, as an
                // empty space does when switched to.
                None => self.insert_page(self.new_tab_index(cx), NEW_TAB, window, cx),
            }
        } else if self.active_tab.is_none() {
            self.insert_page(self.new_tab_index(cx), NEW_TAB, window, cx);
        } else {
            cx.notify();
        }
    }

    /// The tab to show once the active tab at `index` closes, by its index
    /// after the close: its neighbour in the list, after it or else before
    /// it, so closing a tab never moves to another space. A tab not listed,
    /// such as a favourite's, gives way to the list's most recent tab.
    /// `None` when the list has no other tab.
    fn tab_after_closing(&self, index: usize, cx: &Context<Self>) -> Option<usize> {
        let listed = self.listed_tabs(cx);
        let next = match listed.iter().position(|&listed| listed == index) {
            Some(position) => listed
                .iter()
                .skip(position + 1)
                .chain(listed[..position].iter().rev())
                .copied()
                .find(|&candidate| candidate != index),
            None => listed
                .into_iter()
                .filter(|&candidate| candidate != index)
                .max_by_key(|&candidate| self.tabs[candidate].last_active),
        };
        next.map(|next| if next > index { next - 1 } else { next })
    }

    /// Keeps a closed web page so it can be reopened.
    fn remember_closed(&mut self, index: usize, content: &TabContent, cx: &Context<Self>) {
        let url = match content {
            TabContent::Sleeping(sleeping) => {
                (!sleeping.state.url.is_empty()).then(|| sleeping.state.url.clone())
            }
            TabContent::Web(webview) => {
                let view = webview.read(cx);
                view.has_page().then(|| view.state.url.clone())
            }
            TabContent::Page(_) => None,
        };
        let Some(url) = url else { return };
        if self.closed_tabs.len() == CLOSED_TAB_LIMIT {
            self.closed_tabs.remove(0);
        }
        self.closed_tabs.push(ClosedTab { index, url });
    }

    /// Whether a loading tab or a notice shows a spinner.
    fn spinner_shown(&self, cx: &Context<Self>) -> bool {
        self.notice_is_working(cx)
            || self
                .tabs
                .iter()
                .filter_map(|tab| tab.content.webview())
                .any(|webview| {
                    let view = webview.read(cx);
                    view.state.loading && view.has_page()
                })
    }

    /// Whether any tab, and so any spinner, is on screen: in a tab strip
    /// Zen mode has not tucked away, or in a sidebar that is shown or
    /// revealed. Showing them again redraws them at the current step.
    fn tabs_on_screen(&self, cx: &App) -> bool {
        let layout = self.layout(cx);
        (layout.has_tab_strip() && self.sidebar.chrome_shown() > 0.0)
            || (layout.has_sidebar() && (self.sidebar.shown() > 0.0 || self.sidebar.over_page()))
    }

    /// Steps the spinners while any is shown. With reduced motion they stay still.
    pub(super) fn animate_spinner(&mut self, cx: &mut Context<Self>) {
        let animations = Settings::get(cx).animations;
        if self.spinner_running
            || cx.reduce_motion()
            || !animations.enabled
            || !self.spinner_shown(cx)
        {
            return;
        }
        self.spinner_running = true;
        let interval = Duration::from_millis(u64::from(animations.spinner_step_interval_ms()));
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(interval).await;
                let keep_running = update_app(cx, |cx| {
                    this.update(cx, |this, cx| {
                        let keep_running = !cx.reduce_motion()
                            && Settings::get(cx).animations.enabled
                            && this.spinner_shown(cx);
                        if keep_running {
                            this.spinner_step = (this.spinner_step + 1) % LOADING_SPINNER_STEPS;
                            if this.window_visible && this.tabs_on_screen(cx) {
                                this.chrome.update(cx, |_, cx| cx.notify());
                                if this.notice_is_working(cx) {
                                    cx.notify();
                                }
                            }
                        } else {
                            this.spinner_running = false;
                        }
                        keep_running
                    })
                    .ok()
                })
                .await
                .flatten()
                .unwrap_or(false);
                if !keep_running {
                    break;
                }
            }
        })
        .detach();
    }

    /// The arrows along the tab layout's axis move between tabs.
    pub(super) fn tab_key_down(
        &mut self,
        tab_id: u64,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.tab_index_by_id(tab_id) else {
            return;
        };
        if event.keystroke.modifiers.modified() {
            return;
        }
        let Some(offset) = tab_navigation_offset(event.keystroke.key.as_str(), self.tab_axis(cx))
        else {
            return;
        };
        if let Some(next_index) = self.listed_tab_after(index, offset, cx) {
            self.move_tab_focus(next_index, window, cx);
        }
        window.prevent_default();
        cx.stop_propagation();
    }

    /// Whether a tab's icon is still within its appear animation. A newly
    /// revealed icon starts it; the same icon shown again does not.
    pub(super) fn icon_appearing(
        &self,
        tab: u64,
        icon: &TabIcon,
        animations: &super::super::motion::MotionConfig,
    ) -> bool {
        let Some(revealed) = icon.revealed() else {
            return false;
        };
        let mut revealed_icons = self.revealed_icons.borrow_mut();
        match revealed_icons.get(&tab) {
            Some((icon, since)) if *icon == revealed => {
                since.elapsed() < ICON_ENTRANCE.total_duration(animations)
            }
            _ => {
                revealed_icons.insert(tab, (revealed, Instant::now()));
                true
            }
        }
    }

    /// Whether a newly inserted tab is within its one-time entry animation.
    pub(super) fn tab_entry_appearing(
        &self,
        tab: u64,
        animations: &super::super::motion::MotionConfig,
    ) -> bool {
        let mut entries = self.tab_entry_animations.borrow_mut();
        if !animations.allows_motion() {
            entries.remove(&tab);
            return false;
        }
        entries.contains(&tab)
    }
}

fn tab_navigation_offset(key: &str, axis: TabDragAxis) -> Option<isize> {
    match (axis, key) {
        (TabDragAxis::Horizontal, "left") | (TabDragAxis::Vertical, "up") => Some(-1),
        (TabDragAxis::Horizontal, "right") | (TabDragAxis::Vertical, "down") => Some(1),
        _ => None,
    }
}
