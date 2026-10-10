//! Opening the command bar and carrying out what is chosen in it.

use gpui::{AnyElement, ClipboardItem, Context, Focusable, Window, div, prelude::*};
use photon_core::{BrowserCommand, Command, TabLayout};

use super::super::command_bar::{COMMAND_BAR_MOTION, CommandBar, CommandBarEvent, TabSummary};
use super::super::motion::AnimateIn;
use super::super::pages::{SETTINGS, find_page};
use super::super::settings::Settings;
use super::BrowserWindow;
use super::content::TabContent;

impl BrowserWindow {
    /// Opens the command bar over the page, or focuses it when it is open.
    pub(super) fn open_command_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(bar) = &self.command_bar {
            let focus = bar.read(cx).focus_handle(cx);
            window.focus(&focus, cx);
            return;
        }
        let tabs = self.tab_summaries(cx);
        let commands = self.available_commands(cx);
        let bar = cx.new(|cx| CommandBar::new(tabs, commands, window, cx));
        self._command_bar_subscription = Some(cx.subscribe_in(
            &bar,
            window,
            |this, _, event: &CommandBarEvent, window, cx| {
                this.command_bar_event(event, window, cx)
            },
        ));
        self.open_menu = None;
        self.command_bar = Some(bar);
        cx.notify();
    }

    /// The open command bar, or one closing, drawn over the window.
    pub(super) fn command_bar_overlay(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (bar, transition) =
            self.command_bar_presence
                .sync(self.command_bar.clone(), COMMAND_BAR_MOTION, cx)?;
        Some(
            div()
                .absolute()
                .inset_0()
                .child(bar)
                .animate("command-bar-motion", COMMAND_BAR_MOTION, transition)
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
            CommandBarEvent::Open(url) => match find_page(url) {
                Some(page) => self.open_page_tab(page, window, cx),
                None => self.insert_tab(self.tabs.len(), Some(url), window, cx),
            },
            CommandBarEvent::SwitchToTab(index) => self.activate_tab(*index, true, window, cx),
            CommandBarEvent::Run(command) => self.run_command(*command, window, cx),
            CommandBarEvent::Dismiss => {
                // Back to where typing was before it opened.
                self.activate_tab(self.active_tab, true, window, cx);
            }
        }
        cx.notify();
    }

    fn run_command(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        match command {
            Command::NewTab => self.open_tab(window, cx),
            Command::NewWindow => self.dispatch_command(BrowserCommand::NewWindow, window, cx),
            Command::ReopenClosedTab => self.reopen_closed_tab(window, cx),
            Command::CloseTab => self.close_tab(self.active_tab, window, cx),
            Command::ReloadPage => self.dispatch_command(BrowserCommand::Reload, window, cx),
            Command::FindInPage => self.open_find_bar(window, cx),
            Command::CopyAddress => {
                if let Some(webview) = self.active_webview() {
                    let url = webview.read(cx).state.url.clone();
                    cx.write_to_clipboard(ClipboardItem::new_string(url));
                }
                self.activate_tab(self.active_tab, true, window, cx);
            }
            Command::ToggleSidebar => {
                self.toggle_sidebar(cx);
                self.activate_tab(self.active_tab, true, window, cx);
            }
            Command::OpenSettings => self.open_page_tab(SETTINGS, window, cx),
        }
    }

    /// The commands that apply now: page commands only with a web page
    /// showing, and the sidebar only in the vertical layout.
    fn available_commands(&self, cx: &Context<Self>) -> Vec<Command> {
        let has_page = self.active_webview().is_some();
        let vertical = Settings::get(cx).tab_layout == TabLayout::Vertical;
        Command::ALL
            .into_iter()
            .filter(|command| match command {
                Command::ReloadPage | Command::FindInPage | Command::CopyAddress => has_page,
                Command::ReopenClosedTab => !self.closed_tabs.is_empty(),
                Command::ToggleSidebar => vertical,
                _ => true,
            })
            .collect()
    }

    fn tab_summaries(&self, cx: &Context<Self>) -> Vec<TabSummary> {
        self.tabs
            .iter()
            .map(|tab| match &tab.content {
                TabContent::Page(page) => TabSummary {
                    title: page.definition.title.to_owned(),
                    url: tab.content.page_address().unwrap_or_default(),
                },
                TabContent::Web(webview) => {
                    let state = &webview.read(cx).state;
                    TabSummary {
                        title: if state.title.trim().is_empty() {
                            state.url.clone()
                        } else {
                            state.title.clone()
                        },
                        url: state.url.clone(),
                    }
                }
            })
            .collect()
    }
}
