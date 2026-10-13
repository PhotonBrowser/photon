//! The window's tabs as both tab layouts draw them: label, icon and what
//! each interaction does.

use gpui::{App, Context, DispatchPhase, KeyDownEvent, MouseButton, MouseDownEvent, MouseUpEvent};

use super::super::history::BrowsingHistory;
use super::super::pages::PageIcon;
use super::super::settings::Settings;
use super::super::tabs::{TabIcon, TabItem};
use super::BrowserWindow;
use super::content::TabContent;
use std::time::Instant;

impl BrowserWindow {
    /// The tabs the layout lists, in order: in the sidebar, the space's own
    /// tabs but not favourites' tabs, which show as tiles; in the strip,
    /// every tab.
    pub(super) fn listed_tabs(&self, cx: &App) -> Vec<usize> {
        let space = self.shows_spaces(cx).then(|| self.current_space(cx));
        (0..self.tabs.len())
            .filter(|&index| {
                let tab = &self.tabs[index];
                space.is_none_or(|space| tab.space == space && tab.favourite.is_none())
            })
            .collect()
    }

    /// The listed tab `offset` places from the tab at `index`, wrapping at
    /// the ends; from a tab that is not listed, the first or last.
    pub(super) fn listed_tab_after(&self, index: usize, offset: isize, cx: &App) -> Option<usize> {
        let listed = self.listed_tabs(cx);
        let count = listed.len() as isize;
        if count == 0 {
            return None;
        }
        let next = match listed.iter().position(|&listed| listed == index) {
            Some(position) => (position as isize + offset).rem_euclid(count),
            None if offset > 0 => 0,
            None => count - 1,
        };
        Some(listed[next as usize])
    }

    pub(super) fn tab_items(&self, cx: &mut Context<Self>) -> Vec<TabItem> {
        let browser = cx.entity().downgrade();
        let animations = Settings::get(cx).animations;
        let axis = self.tab_axis(cx);
        let suppress_tooltips = self.tab_drag_active();
        self.tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| {
                let (label, icon) = match &tab.content {
                    TabContent::Page(page) => {
                        let icon = match page.definition.icon {
                            PageIcon::Logo => TabIcon::Logo,
                            PageIcon::Symbol(symbol) => TabIcon::Symbol(symbol),
                        };
                        (page.definition.title.to_owned(), icon)
                    }
                    // A tab that has not loaded yet, as when it opens in the
                    // background at launch, shows what it was pinned or
                    // opened as.
                    TabContent::Web(webview)
                        if !webview.read(cx).has_page()
                            && let Some(saved) = tab
                                .pin
                                .clone()
                                .or_else(|| webview.read(cx).requested.clone()) =>
                    {
                        let icon = match BrowsingHistory::favicon(&saved.url, cx) {
                            Some(favicon) => TabIcon::Favicon(favicon),
                            None => TabIcon::Page,
                        };
                        let label = if saved.title.trim().is_empty() {
                            saved.url
                        } else {
                            saved.title
                        };
                        (label, icon)
                    }
                    TabContent::Web(webview) => {
                        let view = webview.read(cx);
                        let icon = if view.audio_playing {
                            TabIcon::Audio {
                                favicon: view.favicon.clone(),
                                muted: view.audio_muted,
                            }
                        } else if view.shows_spinner(&animations) {
                            TabIcon::Loading(self.spinner_step)
                        } else if let Some(favicon) = view.favicon.clone() {
                            TabIcon::Favicon(favicon)
                        } else {
                            TabIcon::Page
                        };
                        let label = if !view.state.title.trim().is_empty() {
                            view.state.title.clone()
                        } else if view.state.url.is_empty() {
                            "New Tab".to_owned()
                        } else {
                            view.state.url.clone()
                        };
                        (label, icon)
                    }
                };
                let stable_id = tab.id;
                let insertion_before = self.incoming_tab_drag.as_ref().and_then(|incoming| {
                    (incoming.pinned == tab.pin.is_some() && incoming.before_id == Some(stable_id))
                        .then_some(incoming.size)
                });
                let insertion_after = self.incoming_tab_drag.as_ref().and_then(|incoming| {
                    (incoming.pinned == tab.pin.is_some() && incoming.after_id == Some(stable_id))
                        .then_some(incoming.size)
                });
                TabItem {
                    id: format!("browser-tab-{stable_id}"),
                    label,
                    icon_appearing: self.icon_appearing(tab.id, &icon, &animations),
                    animations,
                    icon,
                    index,
                    pinned: tab.pin.is_some(),
                    active: self.active_tab == Some(index),
                    focus_handle: self.tab_focus_handles[index].clone(),
                    on_select: Box::new(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        if let Some(until) = this.suppress_tab_click_until.take() {
                            if Instant::now() <= until {
                                return;
                            }
                        }
                        if let Some(index) = this.tab_index_by_id(stable_id) {
                            this.activate_tab(index, true, window, cx);
                        }
                    })),
                    on_key_down: Box::new(cx.listener(
                        move |this, event: &KeyDownEvent, window, cx| {
                            this.tab_key_down(stable_id, event, window, cx);
                        },
                    )),
                    on_close: Box::new(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        if let Some(index) = this.tab_index_by_id(stable_id) {
                            this.close_tab(index, window, cx);
                        }
                    })),
                    on_toggle_audio: Box::new(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        if let Some(webview) = this
                            .tab_index_by_id(stable_id)
                            .and_then(|index| this.tabs[index].content.webview())
                        {
                            webview.update(cx, |view, cx| view.toggle_audio_mute(cx));
                        }
                    })),
                    on_pointer_down: {
                        let browser = browser.clone();
                        let pinned = tab.pin.is_some();
                        Box::new(move |event, phase, hitbox, axis, window, cx| {
                            if phase != DispatchPhase::Bubble
                                || event.button != MouseButton::Left
                                || !hitbox.contains(&event.position)
                            {
                                return;
                            }
                            browser
                                .update(cx, |this, _cx| {
                                    // A previous drag released outside a tab may not
                                    // synthesize a click to consume this one-shot guard.
                                    // A new press starts a fresh gesture either way.
                                    this.suppress_tab_click_until = None;
                                    this.tab_drag_layout = Some(super::TabDragLayout {
                                        id: stable_id,
                                        source_window: window.window_handle(),
                                        axis,
                                        pointer_start: event.position,
                                        pointer_position: event.position,
                                        grab_offset: event.position - hitbox.bounds.origin,
                                        tab_bounds: hitbox.bounds,
                                        pinned,
                                        phase: super::TabDragPhase::PendingDrag,
                                        visual_offset: 0.0,
                                        target_window: None,
                                    });
                                })
                                .ok();
                        })
                    },
                    on_prepaint: {
                        let browser = browser.clone();
                        Box::new(move |bounds, window, cx| {
                            let running = browser
                                .update(cx, |this, _| {
                                    this.tab_geometry.insert(stable_id, bounds);
                                    match this.tab_position_animations.get(&stable_id).copied() {
                                        Some(animation)
                                            if animation.axis == axis && animation.is_running() =>
                                        {
                                            true
                                        }
                                        Some(_) => {
                                            this.tab_position_animations.remove(&stable_id);
                                            false
                                        }
                                        None => false,
                                    }
                                })
                                .unwrap_or(false);
                            if running {
                                window.request_animation_frame();
                            }
                        })
                    },
                    on_drop: self.tab_target_drop_handler(stable_id, cx),
                    position_offset: self
                        .tab_drag_layout
                        .as_ref()
                        .filter(|drag| drag.id == stable_id)
                        .map(|drag| drag.visual_offset)
                        .unwrap_or_else(|| {
                            super::tab_position_animation_offset(
                                self.tab_position_animations.get(&stable_id).copied(),
                                axis,
                            )
                        }),
                    dragging: self.tab_drag_layout.as_ref().is_some_and(|drag| {
                        drag.id == stable_id
                            && !matches!(drag.phase, super::TabDragPhase::PendingDrag)
                    }),
                    suppress_tooltips,
                    insertion_before,
                    insertion_after,
                    on_middle_click: Box::new(cx.listener(
                        move |this, _: &MouseUpEvent, window, cx| {
                            cx.stop_propagation();
                            if let Some(index) = this.tab_index_by_id(stable_id) {
                                this.close_tab(index, window, cx);
                            }
                        },
                    )),
                    on_context_menu: Box::new(cx.listener(
                        move |this, _: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            if let Some(index) = this.tab_index_by_id(stable_id) {
                                this.show_tab_menu(index, window, cx);
                            }
                        },
                    )),
                }
            })
            .collect()
    }
}
