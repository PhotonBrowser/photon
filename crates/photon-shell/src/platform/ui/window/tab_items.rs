//! The window's tabs as both tab layouts draw them: label, icon and what
//! each interaction does.

use gpui::{App, Context, KeyDownEvent, MouseDownEvent, MouseUpEvent};

use super::super::history::BrowsingHistory;
use super::super::pages::PageIcon;
use super::super::tabs::{DraggedTab, TabIcon, TabItem};
use super::BrowserWindow;
use super::content::TabContent;
use super::menu::OpenMenu;

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
                    // A pinned tab that has not loaded yet, as when it opens in
                    // the background at launch, shows what it was pinned as.
                    TabContent::Web(webview)
                        if !webview.read(cx).has_page()
                            && let Some(pin) = &tab.pin =>
                    {
                        let icon = match BrowsingHistory::favicon(&pin.url, cx) {
                            Some(favicon) => TabIcon::Favicon(favicon),
                            None => TabIcon::Page,
                        };
                        (pin.title.clone(), icon)
                    }
                    TabContent::Web(webview) => {
                        let view = webview.read(cx);
                        let icon = if view.audio_playing {
                            TabIcon::Audio {
                                favicon: view.favicon.clone(),
                                muted: view.audio_muted,
                            }
                        } else if view.shows_spinner() {
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
                TabItem {
                    id: format!("browser-tab-{}", tab.id),
                    label,
                    icon_appearing: self.icon_appearing(tab.id, &icon),
                    icon,
                    index,
                    pinned: tab.pin.is_some(),
                    active: index == self.active_tab,
                    focus_handle: self.tab_focus_handles[index].clone(),
                    on_select: Box::new(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.activate_tab(index, true, window, cx);
                    })),
                    on_key_down: Box::new(cx.listener(
                        move |this, event: &KeyDownEvent, window, cx| {
                            this.tab_key_down(index, event, window, cx);
                        },
                    )),
                    on_close: Box::new(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.close_tab(index, window, cx);
                    })),
                    on_toggle_audio: Box::new(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        if let Some(webview) =
                            this.tabs.get(index).and_then(|tab| tab.content.webview())
                        {
                            webview.update(cx, |view, cx| view.toggle_audio_mute(cx));
                        }
                    })),
                    on_middle_click: Box::new(cx.listener(
                        move |this, _: &MouseUpEvent, window, cx| {
                            cx.stop_propagation();
                            this.close_tab(index, window, cx);
                        },
                    )),
                    on_context_menu: Box::new(cx.listener(
                        move |this, event: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            this.open_menu = Some(OpenMenu::Tab(index, event.position));
                            cx.notify();
                        },
                    )),
                    on_drop: Box::new(cx.listener(move |this, dragged: &DraggedTab, _, cx| {
                        this.drop_tab(dragged.index, index, cx);
                    })),
                }
            })
            .collect()
    }
}
