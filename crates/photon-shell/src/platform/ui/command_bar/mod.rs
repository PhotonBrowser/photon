//! The command bar ⌘T opens over the page, as in Arc and Zen: type an
//! address or search to open it in a new tab, find an open tab to switch to,
//! or run a browser command. What it offers comes from
//! `photon_core::command_bar_results`; the window carries out the choice.
//!
//! - `rows`: the result rows.

mod rows;

use gpui::{
    Context, Entity, EventEmitter, FocusHandle, Focusable, MouseButton, Render, Subscription,
    Window, div, prelude::*, px, rgb, rgba,
};
use gpui_elements::editable_text::{
    EditableTextState, StringStorage, TextChanged,
    actions::{Enter, Escape, NavDown, NavUp},
    text_input,
};
use photon_core::{Command, CommandBarResult, OpenTab, SuggestionKind, command_bar_results};
use photon_omnibox::{OmniboxTarget, resolve_with};

use super::controls::themed_text_input;
use super::history::BrowsingHistory;
use super::icons::search_icon_sized;
use super::layout::{Elevated, Elevation, Raised, h_stack, v_stack};
use super::motion::Entrance;
use super::settings::Settings;
use super::{metrics, theme::palette};

/// How the command bar appears.
pub(super) const COMMAND_BAR_MOTION: Entrance = Entrance::popover();

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
        window.focus(&input.focus_handle(cx), cx);
        let mut bar = Self {
            input,
            tabs,
            commands,
            results: Vec::new(),
            selected: 0,
            _input_subscriptions: input_subscriptions,
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
        self.selected = self.selected.saturating_sub(1);
        cx.notify();
    }

    fn select_next(&mut self, _: &NavDown, _: &mut Window, cx: &mut Context<Self>) {
        self.selected = (self.selected + 1).min(self.results.len().saturating_sub(1));
        cx.notify();
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
            .w(px(metrics::COMMAND_BAR_WIDTH))
            .max_w_full()
            .rounded(px(metrics::COMMAND_BAR_RADIUS))
            .border_1()
            .border_color(rgba(palette.menu_border))
            .raised(palette)
            .text_size(px(metrics::COMMAND_BAR_FONT_SIZE))
            .text_color(rgb(palette.text_primary))
            .elevated(Elevation::High)
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .capture_action(cx.listener(Self::submit))
            .capture_action(cx.listener(Self::dismiss))
            .capture_action(cx.listener(Self::select_previous))
            .capture_action(cx.listener(Self::select_next))
            .child(field)
            .children(rows);
        // Clicking anywhere else closes it.
        div()
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .items_center()
            .pt(px(metrics::COMMAND_BAR_TOP))
            .px(px(metrics::PAGE_INSET))
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| cx.emit(CommandBarEvent::Dismiss)),
            )
            .child(panel)
    }
}
