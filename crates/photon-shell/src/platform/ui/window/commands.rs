//! Running commands from shortcuts, menus and other browser controls.

use gpui::{App, ClipboardItem, Context, Window};
use photon_core::{BrowserCommand, Command, CommandAvailability, WindowSession};

use super::super::super::engine::ZoomStep;
use super::super::pages::{DOWNLOADS, HISTORY, SETTINGS};
use super::super::settings::Settings;
use super::BrowserWindow;

impl BrowserWindow {
    /// Runs a registered browser command from a shortcut, menu or control.
    pub(super) fn execute_command(
        &mut self,
        command: Command,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match command {
            Command::NewTab => self.open_tab(window, cx),
            Command::NewWindow => self.dispatch_command(BrowserCommand::NewWindow, window, cx),
            Command::ReopenClosedTab => self.reopen_closed_tab(window, cx),
            Command::CloseTab => {
                if let Some(active_tab) = self.active_tab {
                    self.close_tab(active_tab, window, cx);
                } else if self.tabs.is_empty() && !Self::closes_with_last_tab(window, cx) {
                    // Closes this window only; other windows stay open.
                    self.record_session_state(WindowSession::default(), cx);
                    window.remove_window();
                }
            }
            Command::ReloadPage => self.dispatch_command(BrowserCommand::Reload, window, cx),
            Command::FindInPage => self.open_find_bar(window, cx),
            Command::CopyAddress => {
                if let Some(webview) = self.active_webview() {
                    let url = webview.read(cx).state.url.clone();
                    cx.write_to_clipboard(ClipboardItem::new_string(url));
                }
            }
            Command::ToggleSidebar | Command::ToggleSidebarIcons | Command::ToggleZenMode => {
                match command {
                    Command::ToggleSidebar => self.toggle_sidebar(cx),
                    Command::ToggleSidebarIcons => self.toggle_sidebar_icons(cx),
                    _ => self.toggle_zen_mode(cx),
                }
            }
            Command::NewSpace => self.edit_space(None, window, cx),
            Command::NextSpace => self.switch_space_by(1, cx),
            Command::PreviousSpace => self.switch_space_by(-1, cx),
            Command::OpenSettings => self.open_page_tab(SETTINGS, window, cx),
            Command::OpenHistory => self.open_page_tab(HISTORY, window, cx),
            Command::OpenDownloads => self.open_page_tab(DOWNLOADS, window, cx),
            Command::OpenTaskManager => {
                let runtime = self.runtime.clone();
                cx.defer(move |cx| super::task_manager::open_task_manager(runtime, cx));
            }
            Command::ShowKeyboardShortcuts => self.show_keyboard_shortcuts(window, cx),
            Command::StopLoading => self.dispatch_command(BrowserCommand::StopLoading, window, cx),
            Command::GoBack => self.dispatch_command(BrowserCommand::Back, window, cx),
            Command::GoForward => self.dispatch_command(BrowserCommand::Forward, window, cx),
            Command::FocusAddressField => {
                self.show_address_field(cx);
                self.omnibox
                    .update(cx, |omnibox, cx| omnibox.focus(window, cx));
            }
            Command::SelectNextTab => self.select_relative_tab(1, window, cx),
            Command::SelectPreviousTab => self.select_relative_tab(-1, window, cx),
            Command::ZoomIn => self.zoom(ZoomStep::In, cx),
            Command::ZoomOut => self.zoom(ZoomStep::Out, cx),
            Command::ResetZoom => self.zoom(ZoomStep::Reset, cx),
        }
    }

    /// Capabilities used to validate registered commands for this window.
    pub(super) fn command_availability(&self, cx: &App) -> CommandAvailability {
        let layout = self.layout(cx);
        let shows_spaces = layout.shows_spaces();
        let spaces = &Settings::get(cx).spaces;
        let active_webview = self.active_webview();
        let page_state = active_webview
            .as_ref()
            .map(|webview| &webview.read(cx).state);
        CommandAvailability {
            has_page: active_webview.is_some(),
            has_multiple_tabs: self.listed_tabs(cx).len() > 1,
            has_closed_tabs: !self.closed_tabs.is_empty(),
            can_go_back: page_state.is_some_and(|state| state.can_go_back),
            can_go_forward: page_state.is_some_and(|state| state.can_go_forward),
            is_loading: page_state.is_some_and(|state| state.loading),
            has_sidebar: layout.has_sidebar(),
            shows_spaces,
            has_next_space: shows_spaces
                && spaces.neighbour_of(self.current_space(cx), 1).is_some(),
            has_previous_space: shows_spaces
                && spaces.neighbour_of(self.current_space(cx), -1).is_some(),
        }
    }
}
