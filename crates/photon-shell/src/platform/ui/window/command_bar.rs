//! Opening the command bar and carrying out what is chosen in it.

use gpui::{AnyElement, App, ClipboardItem, Context, Focusable, Window, div, prelude::*};
use photon_core::{BrowserCommand, Command, CommandAvailability};

use super::super::command_bar::{COMMAND_BAR_MOTION, CommandBar, CommandBarEvent, TabSummary};
use super::super::motion::AnimateIn;
use super::super::pages::SETTINGS;
use super::super::settings::Settings;
use super::super::workspace_layout::WorkspaceLayout;
use super::BrowserWindow;
use super::content::{BrowserTab, TabContent};

impl BrowserWindow {
    /// Opens the command bar over the page, or focuses it when it is open.
    pub(super) fn open_command_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(bar) = &self.command_bar {
            let focus = bar.read(cx).focus_handle(cx);
            window.focus(&focus, cx);
            return;
        }
        let tabs = self.tab_summaries(cx);
        let commands = self.command_availability(cx).commands().collect();
        let bar = cx.new(|cx| CommandBar::new(tabs, commands, window, cx));
        self._command_bar_subscription = Some(cx.subscribe_in(
            &bar,
            window,
            |this, _, event: &CommandBarEvent, window, cx| {
                this.command_bar_event(event, window, cx)
            },
        ));
        self.command_bar = Some(bar);
        cx.notify();
    }

    /// The open command bar, or one closing, drawn over the window.
    pub(super) fn command_bar_overlay(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if let Some(bar) = self.command_bar.clone() {
            let commands = self.command_availability(cx).commands().collect();
            bar.update(cx, |bar, cx| bar.update_commands(commands, cx));
        }

        let animations = super::super::settings::Settings::motion(cx);
        let (bar, transition) = self.command_bar_presence.sync(
            self.command_bar.clone(),
            COMMAND_BAR_MOTION,
            &animations,
            cx,
        )?;
        Some(
            div()
                .absolute()
                .inset_0()
                .child(bar)
                .animate(
                    "command-bar-motion",
                    COMMAND_BAR_MOTION,
                    transition,
                    &super::super::settings::Settings::motion(cx),
                )
                .into_any_element(),
        )
    }

    fn command_bar_event(
        &mut self,
        event: &CommandBarEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.command_bar = None;
        self._command_bar_subscription = None;
        match event {
            CommandBarEvent::Open(url) => self.open_in_new_tab(url, window, cx),
            CommandBarEvent::SwitchToTab(index) => self.activate_tab(*index, true, window, cx),
            CommandBarEvent::Run(command) => {
                let available = self.command_availability(cx).allows(*command);
                if available {
                    self.execute_command(*command, window, cx);
                }
                if available
                    && matches!(
                        command,
                        Command::CopyAddress
                            | Command::ToggleSidebar
                            | Command::ToggleSidebarIcons
                            | Command::ToggleZenMode
                    )
                    && let Some(active_tab) = self.active_tab
                {
                    self.activate_tab(active_tab, true, window, cx);
                }
            }
            CommandBarEvent::Dismiss => {
                // Back to where typing was before it opened.
                if let Some(active_tab) = self.active_tab {
                    self.activate_tab(active_tab, true, window, cx);
                } else {
                    self.omnibox
                        .update(cx, |omnibox, cx| omnibox.focus(window, cx));
                }
            }
        }
        cx.notify();
    }

    /// Runs a browser command from the command bar, a shortcut or a menu.
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
                } else if self.tabs.is_empty() && !Settings::get(cx).close_window_with_last_tab {
                    cx.quit();
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
            Command::ShowKeyboardShortcuts => self.show_keyboard_shortcuts(window, cx),
        }
    }

    /// Capabilities used to offer and validate commands in this window.
    pub(super) fn command_availability(&self, cx: &App) -> CommandAvailability {
        let layout = WorkspaceLayout::current(cx);
        let shows_spaces = layout.shows_spaces();
        let spaces = &Settings::get(cx).spaces;
        CommandAvailability {
            has_page: self.active_webview().is_some(),
            has_closed_tabs: !self.closed_tabs.is_empty(),
            has_sidebar: layout.has_sidebar(),
            shows_spaces,
            has_next_space: shows_spaces && spaces.neighbour(1).is_some(),
            has_previous_space: shows_spaces && spaces.neighbour(-1).is_some(),
        }
    }

    fn tab_summaries(&self, cx: &Context<Self>) -> Vec<TabSummary> {
        self.tabs.iter().map(|tab| tab_summary(tab, cx)).collect()
    }
}

/// A tab's title, or its address when the page has none, and its address.
pub(super) fn tab_summary(tab: &BrowserTab, cx: &App) -> TabSummary {
    match &tab.content {
        TabContent::Page(page) => TabSummary {
            title: page.definition.title.to_owned(),
            url: tab.content.page_address().unwrap_or_default(),
        },
        TabContent::Sleeping(sleeping) => {
            let state = &sleeping.state;
            TabSummary {
                title: if state.title.trim().is_empty() {
                    state.url.clone()
                } else {
                    state.title.clone()
                },
                url: state.url.clone(),
            }
        }
        TabContent::Web(webview) => {
            let view = webview.read(cx);
            // Until the page loads, it is what the tab was pinned or opened as.
            if !view.has_page()
                && let Some(saved) = tab.pin.as_ref().or(view.requested.as_ref())
            {
                return TabSummary {
                    title: if saved.title.trim().is_empty() {
                        saved.url.clone()
                    } else {
                        saved.title.clone()
                    },
                    url: saved.url.clone(),
                };
            }
            let state = &view.state;
            TabSummary {
                title: if state.title.trim().is_empty() {
                    state.url.clone()
                } else {
                    state.title.clone()
                },
                url: state.url.clone(),
            }
        }
    }
}
