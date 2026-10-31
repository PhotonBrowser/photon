//! What the browser's context menus do to a particular tab, favourite or
//! space. A menu item carries its command in a [`RunMenuCommand`] action,
//! which the window runs when the item is chosen.

use gpui::{App, Context, MenuItem, SharedString, Window};
use photon_core::{ClearBrowsingData, Shortcut, SpaceId};

use super::BrowserWindow;

/// Something a context menu does to the tab, favourite or space it was
/// opened for.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum MenuCommand {
    ReloadTab(usize),
    DuplicateTab(usize),
    TogglePin(usize),
    AddFavourite { tab: usize, shortcut: Shortcut },
    RemoveFavourite { url: String, space: SpaceId },
    CloseTab(usize),
    CloseOtherTabs(usize),
    CloseTabsToRight(usize),
    EditSpace(SpaceId),
    EditTheme(SpaceId),
    DeleteSpace(SpaceId),
    // TODO: Scope these to the site whose info popover opened the menu;
    // clear_browsing_data currently clears data for the whole profile.
    ClearCache,
    ClearCookies,
}

impl MenuCommand {
    /// Whether the command changes how Photon is set up, which a private
    /// window may not do.
    fn customises(&self) -> bool {
        matches!(
            self,
            Self::EditSpace(_) | Self::EditTheme(_) | Self::DeleteSpace(_)
        )
    }
}

/// Runs a context menu's command.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = photon, no_json)]
pub(super) struct RunMenuCommand {
    command: MenuCommand,
}

/// A menu item that runs `command`.
pub(super) fn command_item(name: impl Into<SharedString>, command: MenuCommand) -> MenuItem {
    MenuItem::action(name, RunMenuCommand { command })
}

impl BrowserWindow {
    pub(super) fn run_menu_command(
        &mut self,
        item: &RunMenuCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // A private window is disposable and cannot change how Photon is set up.
        if self.private && item.command.customises() {
            return;
        }
        match item.command.clone() {
            MenuCommand::ReloadTab(tab) => self.reload_tab(tab, window, cx),
            MenuCommand::DuplicateTab(tab) => self.duplicate_tab(tab, window, cx),
            MenuCommand::TogglePin(tab) => self.toggle_pin(tab, cx),
            MenuCommand::AddFavourite { tab, shortcut } => self.add_favourite(tab, shortcut, cx),
            MenuCommand::RemoveFavourite { url, space } => self.remove_favourite(&url, space, cx),
            MenuCommand::CloseTab(tab) => self.close_tab(tab, window, cx),
            MenuCommand::CloseOtherTabs(tab) => self.close_other_tabs(tab, window, cx),
            MenuCommand::CloseTabsToRight(tab) => self.close_tabs_to_right(tab, window, cx),
            MenuCommand::EditSpace(space) => self.edit_space(Some(space), window, cx),
            MenuCommand::EditTheme(space) => self.show_theme_picker(space, window, cx),
            MenuCommand::DeleteSpace(space) => self.delete_space(space, window, cx),
            MenuCommand::ClearCache => self.clear_data(
                ClearBrowsingData {
                    cache: true,
                    ..ClearBrowsingData::default()
                },
                "Cache cleared",
                cx,
            ),
            MenuCommand::ClearCookies => self.clear_data(
                ClearBrowsingData {
                    site_data: true,
                    ..ClearBrowsingData::default()
                },
                "Cookies and site data cleared",
                cx,
            ),
        }
    }

    fn clear_data(&self, request: ClearBrowsingData, message: &'static str, cx: &mut App) {
        let runtime = self.runtime.clone();
        super::super::history::BrowsingHistory::clear(
            request,
            &runtime,
            move |cx| {
                super::super::show_alert(cx, message);
            },
            cx,
        );
    }

    pub(super) fn toggle_performance_overlay(&mut self, cx: &mut Context<Self>) {
        if let Some(webview) = self.active_webview() {
            webview.update(cx, |view, cx| {
                view.set_performance_overlay_enabled(!view.performance_overlay_enabled);
                cx.notify();
            });
        }
        cx.notify();
    }
}
