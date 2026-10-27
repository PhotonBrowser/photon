//! A tab's and a favourite's right-click menus, and the tab actions behind
//! them.

use crate::platform::shortcuts::CloseTab;
use gpui::{Context, MenuItem, Window};
use photon_core::BrowserCommand;

use super::BrowserWindow;
use super::content::TabContent;
use super::menu::{MenuCommand, command_item};

impl BrowserWindow {
    /// Moves the tab at `from` to `to`, keeping the same tab active. Callers
    /// keep pinned tabs first.
    pub(super) fn move_tab_to(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        let count = self.tabs.len();
        if from == to || from >= count || to >= count {
            return;
        }
        let active = self.active_tab.map(|index| self.tabs[index].id);
        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);
        let subscription = self.tab_subscriptions.remove(from);
        self.tab_subscriptions.insert(to, subscription);
        let focus_handle = self.tab_focus_handles.remove(from);
        self.tab_focus_handles.insert(to, focus_handle);
        self.active_tab = active.and_then(|id| self.tabs.iter().position(|tab| tab.id == id));
        cx.notify();
    }

    /// Opens the tab's page again in a new tab beside it.
    pub(super) fn duplicate_tab(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.tabs.get(index) else {
            return;
        };
        match &tab.content {
            TabContent::Web(webview) => {
                let view = webview.read(cx);
                let address = view.has_page().then(|| view.state.url.clone());
                self.insert_tab(index + 1, address.as_deref(), window, cx);
            }
            TabContent::Page(page) => {
                let page = page.definition;
                self.insert_page(index + 1, page, window, cx);
            }
            TabContent::Sleeping(sleeping) => {
                let address = sleeping.state.url.clone();
                self.insert_tab(index + 1, Some(&address), window, cx);
            }
        }
    }

    pub(super) fn reload_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .tabs
            .get(index)
            .is_some_and(|tab| matches!(&tab.content, TabContent::Sleeping(_)))
        {
            self.activate_tab(index, true, window, cx);
            return;
        }
        match self.tabs.get(index).and_then(|tab| tab.content.webview()) {
            Some(tab) => tab.update(cx, |view, cx| {
                if let Err(error) = view.execute(BrowserCommand::Reload, cx) {
                    super::super::super::trace(format_args!("reloading tab: {error:#}"));
                }
            }),
            None => cx.notify(),
        }
    }

    /// The everyday tabs listed beside the tab at `index`: neither pinned
    /// nor favourites', in this space. With `after_only`, only those listed
    /// after it. Bulk closing touches no others, so it never unpins a tab or
    /// closes one the sidebar does not show.
    fn closable_neighbours(
        &self,
        index: usize,
        after_only: bool,
        cx: &Context<Self>,
    ) -> Vec<usize> {
        let listed = self.listed_tabs(cx);
        let position = listed.iter().position(|&listed| listed == index);
        listed
            .iter()
            .enumerate()
            .filter(|&(at, &other)| {
                other != index
                    && self.tabs[other].pin.is_none()
                    && (!after_only || position.is_some_and(|position| at > position))
            })
            .map(|(_, &other)| other)
            .collect()
    }

    /// Closes the other everyday tabs beside the one at `index`, which
    /// becomes active.
    pub(super) fn close_other_tabs(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.tabs.len() {
            return;
        }
        let others = self.closable_neighbours(index, false, cx);
        self.activate_tab(index, true, window, cx);
        self.close_tabs(others, window, cx);
    }

    /// Closes the everyday tabs listed after the one at `index`.
    pub(super) fn close_tabs_to_right(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.tabs.len() {
            return;
        }
        let right = self.closable_neighbours(index, true, cx);
        if self
            .active_tab
            .is_some_and(|active| right.contains(&active))
        {
            self.activate_tab(index, true, window, cx);
        }
        self.close_tabs(right, window, cx);
    }

    /// Closes the tabs at `indices`, last first so the others keep their
    /// places.
    fn close_tabs(&mut self, mut indices: Vec<usize>, window: &mut Window, cx: &mut Context<Self>) {
        indices.sort_unstable_by(|a, b| b.cmp(a));
        for index in indices {
            self.close_tab(index, window, cx);
        }
    }

    /// Opens the shared GPUI menu for the tab at `index`.
    pub(super) fn show_tab_menu(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let items = self.tab_menu_items(index, cx);
        self.show_context_menu(window, items, cx);
    }

    /// The menu for the tab at `index`: reloading, duplicating, pinning and
    /// favouriting it, and closing it or others.
    fn tab_menu_items(&self, index: usize, cx: &Context<Self>) -> Vec<MenuItem> {
        let mut items = vec![
            command_item("Reload Tab", MenuCommand::ReloadTab(index)),
            command_item("Duplicate Tab", MenuCommand::DuplicateTab(index)),
        ];
        if self.can_pin(index, cx) {
            let pinned = self.tabs[index].pin.is_some();
            items.push(command_item(
                if pinned { "Unpin Tab" } else { "Pin Tab" },
                MenuCommand::TogglePin(index),
            ));
        }
        items.extend(self.favourite_item(index, cx));
        items.push(MenuItem::separator());
        // ⌘W closes the active tab, so its item is that shortcut's action
        // and shows its keys.
        items.push(if self.active_tab == Some(index) {
            MenuItem::action("Close Tab", CloseTab)
        } else {
            command_item("Close Tab", MenuCommand::CloseTab(index))
        });
        if !self.closable_neighbours(index, false, cx).is_empty() {
            items.push(command_item(
                "Close Other Tabs",
                MenuCommand::CloseOtherTabs(index),
            ));
        }
        if !self.closable_neighbours(index, true, cx).is_empty() {
            items.push(command_item(
                "Close Tabs to the Right",
                MenuCommand::CloseTabsToRight(index),
            ));
        }
        items
    }
}
