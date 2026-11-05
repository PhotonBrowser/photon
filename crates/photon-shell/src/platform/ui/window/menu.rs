//! What the browser's context menus do to a particular tab, favourite or
//! space. A menu item carries its command in a [`RunMenuCommand`] action,
//! which the window runs when the item is chosen.

use gpui::{App, Context, MenuItem, SharedString, Window};
use photon_core::{Feature, Shortcut, SpaceId};

use crate::platform::app_update::update_app;

use super::BrowserWindow;

/// Something a context menu does to the tab, favourite or space it was
/// opened for.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum MenuCommand {
    ReloadTab(usize),
    DuplicateTab(usize),
    TogglePin(usize),
    AddFavourite {
        tab: usize,
        shortcut: Shortcut,
    },
    RemoveFavourite {
        url: String,
        space: SpaceId,
    },
    CloseTab(usize),
    CloseOtherTabs(usize),
    CloseTabsToRight(usize),
    EditSpace(SpaceId),
    EditTheme(SpaceId),
    DeleteSpace(SpaceId),
    /// Switches to the tab, and to its space.
    ActivateTab(usize),
    /// Mutes or unmutes the tab's sound.
    ToggleTabAudio(usize),
    /// Shows the tab beside the active one.
    SplitWithTab(usize),
    /// Shows the split's tabs one at a time again.
    CloseSplitView,
    /// Opens an empty pane beside the tab, to choose what it shows.
    CreateSplitView(usize),
    /// Deletes the cookies and web storage of the site with this host.
    ClearSiteData(String),
}

impl MenuCommand {
    /// Whether the command changes how Photon is set up, which a private
    /// window may not do.
    fn customises(&self) -> bool {
        matches!(
            self,
            Self::EditSpace(_) | Self::EditTheme(_) | Self::DeleteSpace(_) | Self::ClearSiteData(_)
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
            MenuCommand::ActivateTab(tab) => self.activate_tab(tab, true, window, cx),
            MenuCommand::ToggleTabAudio(tab) => {
                if let Some(webview) = self.tabs.get(tab).and_then(|tab| tab.content.webview()) {
                    webview.update(cx, |view, cx| view.toggle_audio_mute(cx));
                }
            }
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
            MenuCommand::SplitWithTab(tab) => self.split_with_tab(tab, window, cx),
            MenuCommand::CloseSplitView => self.close_split_view(true, cx),
            MenuCommand::CreateSplitView(tab) => self.create_split_view(tab, window, cx),
            MenuCommand::ClearSiteData(host) => self.clear_site_data(&host, cx),
        }
    }

    fn clear_site_data(&self, host: &str, cx: &mut App) {
        let app = cx.to_async();
        self.runtime.clear_site_data(host, move || {
            app.spawn(async move |cx| {
                update_app(cx, |cx| {
                    super::super::show_alert(cx, "Cookies and site data cleared")
                })
                .await
            })
            .detach();
        });
    }

    pub(super) fn toggle_performance_overlay(&mut self, cx: &mut Context<Self>) {
        if !super::super::settings::Settings::get(cx)
            .features
            .is_enabled(Feature::PerformanceTools)
        {
            return;
        }
        if let Some(webview) = self.active_webview() {
            webview.update(cx, |view, cx| {
                view.set_performance_overlay_enabled(!view.performance_overlay_enabled);
                cx.notify();
            });
        }
        cx.notify();
    }
}
