//! The command bar ⌘T opens over the page, as in Arc and Zen: type an
//! address or search to open it in a new tab, find an open tab to switch to,
//! or run a browser command. What it offers comes from
//! `photon_core::command_bar_results`; the window carries out the choice.
//!
//! - `rows`: the result rows.

mod rows;

use gpui::{
    Context, Entity, EventEmitter, FocusHandle, Focusable, Render, Subscription, Window, div,
    prelude::*, px, relative, rgb, rgba,
};
use gpui_elements::editable_text::{
    EditableTextState, StringStorage, TextChanged,
    actions::{Enter, Escape, NavDown, NavUp},
    text_input,
};
use photon_core::{Command, CommandBarResult, OpenTab, SuggestionKind, command_bar_results};
use photon_omnibox::{OmniboxTarget, resolve_with};

use super::controls::OwnsClicks;
use super::controls::themed_text_input;
use super::history::BrowsingHistory;
use super::icons::search_icon_sized;
use super::layout::{Elevated, Elevation, Raised, h_stack, v_stack};
use super::motion::{Entrance, popover};
use super::settings::Settings;
use super::{metrics, theme::palette};

/// How the command bar appears.
pub(super) const COMMAND_BAR_MOTION: Entrance = popover();

/// An open tab, as the command bar offers it.
pub(super) struct TabSummary {
    pub(super) title: String,
    pub(super) url: String,
}

/// What the command bar asks its window to do. Each closes it.
pub(super) enum CommandBarEvent {
    /// Open this address in a new tab.
    Open(String),
    SwitchToTab(usize),
    Run(Command),
    /// Close without doing anything.
    Dismiss,
}

impl EventEmitter<CommandBarEvent> for CommandBar {}

pub(super) struct CommandBar {
    input: Entity<EditableTextState>,
    tabs: Vec<TabSummary>,
    commands: Vec<Command>,
    results: Vec<CommandBarResult>,
    /// The row Enter chooses.
    selected: usize,
    _input_subscriptions: [Subscription; 2],
    _history_subscription: Subscription,
    _keystroke_subscription: Subscription,
}

impl CommandBar {
    /// A command bar offering `tabs` and `commands`, ready to type in.
    pub(super) fn new(
        tabs: Vec<TabSummary>,
        commands: Vec<Command>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| EditableTextState::new(StringStorage::default(), cx));
        let input_subscriptions = [
            cx.subscribe(&input, |this, _, _: &TextChanged, cx| {
                this.update_results(cx)
            }),
            cx.observe(&input, |_, _, cx| cx.notify()),
        ];
        let history_subscription =
            cx.observe_global::<BrowsingHistory>(|this, cx| this.update_results(cx));
        let window_id = window.window_handle().window_id().as_u64();
        let this = cx.entity().downgrade();
        let keystroke_subscription = cx.intercept_keystrokes(move |event, window, cx| {
            if window.window_handle().window_id().as_u64() != window_id {
                return;
            }

            this.update(cx, |this, cx| {
                if !this.input.focus_handle(cx).is_focused(window)
                    || event.keystroke.modifiers.modified()
                {
                    return;
                }

                let handled = match event.keystroke.key.as_str() {
                    "up" => this.move_selection(false, cx),
                    "down" => this.move_selection(true, cx),
                    _ => false,
                };
                if handled {
                    cx.stop_propagation();
                }
            })
            .ok();
        });
        window.focus(&input.focus_handle(cx), cx);
        let mut bar = Self {
            input,
            tabs,
            commands,
            results: Vec::new(),
            selected: 0,
            _input_subscriptions: input_subscriptions,
            _history_subscription: history_subscription,
            _keystroke_subscription: keystroke_subscription,
        };
        bar.update_results(cx);
        bar
    }

    fn update_results(&mut self, cx: &mut Context<Self>) {
        let query = self.input.read(cx).as_str().to_owned();
        let engines = Settings::search_engines(cx);
        let suggestions = if query.trim().is_empty() {
            Vec::new()
        } else {
            BrowsingHistory::suggestions(&query, &engines, cx).rows
        };
        let tabs: Vec<OpenTab> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| OpenTab {
                index,
                title: &tab.title,
                url: &tab.url,
            })
            .collect();
        self.results = command_bar_results(&query, &tabs, &self.commands, suggestions);
        self.selected = 0;
        cx.notify();
    }

    /// Refreshes the commands offered by the owning browser window.
    pub(super) fn update_commands(&mut self, commands: Vec<Command>, cx: &mut Context<Self>) {
        if self.commands == commands {
            return;
        }

        let selected = self.results.get(self.selected).cloned();
        self.commands = commands;
        self.update_results(cx);
        if let Some(selected) = selected
            && let Some(index) = self.results.iter().position(|result| *result == selected)
        {
            self.selected = index;
        }
    }

    /// Carries out the row at `index`.
    fn choose(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(result) = self.results.get(index).cloned() else {
            return;
        };
        let event = match result {
            CommandBarResult::Open(suggestion) => {
                if matches!(
                    suggestion.kind,
                    SuggestionKind::Search { .. } | SuggestionKind::PastSearch
                ) {
                    BrowsingHistory::record_search(&suggestion.text, cx);
                }
                CommandBarEvent::Open(suggestion.url)
            }
            CommandBarResult::SwitchToTab { tab, .. } => CommandBarEvent::SwitchToTab(tab),
            CommandBarResult::Run { command, .. } => CommandBarEvent::Run(command),
        };
        cx.emit(event);
    }

    fn submit(&mut self, _: &Enter, _: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        if !self.results.is_empty() {
            self.choose(self.selected, cx);
            return;
        }
        let text = self.input.read(cx).as_str().trim().to_owned();
        if let Ok(target) = resolve_with(&text, &Settings::search_engines(cx)) {
            if let OmniboxTarget::Search { query, .. } = &target {
                BrowsingHistory::record_search(query, cx);
            }
            cx.emit(CommandBarEvent::Open(target.url().to_owned()));
        }
    }

    fn dismiss(&mut self, _: &Escape, _: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        cx.emit(CommandBarEvent::Dismiss);
    }

    fn select_previous(&mut self, _: &NavUp, _: &mut Window, cx: &mut Context<Self>) {
        if !self.move_selection(false, cx) {
            cx.propagate();
        } else {
            cx.stop_propagation();
        }
    }

    fn select_next(&mut self, _: &NavDown, _: &mut Window, cx: &mut Context<Self>) {
        if !self.move_selection(true, cx) {
            cx.propagate();
        } else {
            cx.stop_propagation();
        }
    }

    fn move_selection(&mut self, next: bool, cx: &mut Context<Self>) -> bool {
        if self.results.is_empty() {
            return false;
        }

        let selected = if next {
            (self.selected + 1).min(self.results.len() - 1)
        } else {
            self.selected.saturating_sub(1)
        };
        if selected != self.selected {
            self.selected = selected;
            cx.notify();
        }
        true
    }

    fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.selected != index {
            self.selected = index;
            cx.notify();
        }
    }
}

impl Focusable for CommandBar {
    fn focus_handle(&self, cx: &gpui::App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}

impl Render for CommandBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = palette(window, cx);
        let field = h_stack()
            .items_center()
            .gap(px(metrics::OMNIBOX_GAP))
            .h(px(metrics::COMMAND_BAR_FIELD_HEIGHT))
            .px(px(metrics::COMMAND_BAR_FIELD_PADDING))
            .flex_shrink_0()
            .child(search_icon_sized(
                palette.text_secondary,
                metrics::COMMAND_BAR_ICON_SIZE,
            ))
            .child(
                themed_text_input(text_input("command-bar-input"), palette)
                    .state(self.input.downgrade())
                    .track_focus(&self.input.focus_handle(cx))
                    .placeholder("Search, enter an address, or find a tab or command")
                    .placeholder_color(rgb(palette.text_secondary))
                    .flex_1()
                    .min_w_0()
                    .whitespace_nowrap()
                    .overflow_x_scroll(),
            );
        let rows = if self.results.is_empty() {
            None
        } else {
            let this = cx.entity().downgrade();
            let hover = this.clone();
            Some(rows::result_rows(
                &self.results,
                self.selected,
                palette,
                move |index, _, cx| {
                    this.update(cx, |this, cx| this.choose(index, cx)).ok();
                },
                move |index, _, cx| {
                    hover.update(cx, |this, cx| this.select(index, cx)).ok();
                },
                cx,
            ))
        };
        let panel = v_stack()
            .id("command-bar")
            .absolute()
            .top_0()
            .w_full()
            .rounded_t(px(metrics::command_bar_top_radius()))
            .rounded_b(px(metrics::command_bar_bottom_radius()))
            .border_1()
            .border_color(rgba(palette.menu_border))
            .raised(palette)
            .text_size(px(metrics::COMMAND_BAR_FONT_SIZE))
            .text_color(rgb(palette.text_primary))
            .elevated(Elevation::High)
            .occlude()
            .on_mouse_down_out(cx.listener(|_, _, _, cx| cx.emit(CommandBarEvent::Dismiss)))
            .owns_clicks()
            .capture_action(cx.listener(Self::submit))
            .capture_action(cx.listener(Self::dismiss))
            .capture_action(cx.listener(Self::select_previous))
            .capture_action(cx.listener(Self::select_next))
            .child(field)
            .children(rows);
        // The field sits a little above the middle of the window, and results
        // grow down from it, so it stays put as they change. Clicking anywhere
        // else closes it.
        let anchor = div()
            .relative()
            .w(px(metrics::COMMAND_BAR_WIDTH))
            .max_w_full()
            .h(px(metrics::COMMAND_BAR_FIELD_HEIGHT))
            .child(panel);
        div()
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .items_center()
            .px(px(metrics::PAGE_INSET))
            .occlude()
            .child(
                div()
                    .flex_shrink_0()
                    .h(relative(metrics::COMMAND_BAR_TOP_FRACTION)),
            )
            .child(anchor)
    }
}
