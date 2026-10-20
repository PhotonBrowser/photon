//! A keyboard-first launcher for registered browser commands.

use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight, HighlightStyle,
    MouseButton, Render, Role, StyledText, Subscription, Window, div, prelude::*, px, rgb, rgba,
};
use gpui_elements::editable_text::{
    EditableTextState, StringStorage, TextChanged,
    actions::{Enter, Escape, NavDown, NavUp},
    text_input,
};
use photon_core::{COMMAND_REGISTRY, Command, CommandAvailability, CommandMatch};
use photon_shortcuts::action_for_command;

use super::controls::{OwnsClicks, themed_text_input};
use super::icons::search_icon_sized;
use super::kbd::{action_kbd, keycap_label};
use super::layout::{Elevated, Raised, h_stack, v_stack};
use super::metrics;
use super::settings::Settings;

pub(super) const COMMAND_PALETTE_MOTION: super::motion::Entrance = super::motion::popover();

pub(super) enum CommandPaletteEvent {
    Run(Command),
    Dismiss,
}

impl EventEmitter<CommandPaletteEvent> for CommandPalette {}

pub(super) struct CommandPalette {
    input: Entity<EditableTextState>,
    focus_handle: FocusHandle,
    availability: CommandAvailability,
    matches: Vec<CommandMatch>,
    selected: usize,
    _input_subscriptions: [Subscription; 2],
}

impl CommandPalette {
    pub(super) fn new(
        availability: CommandAvailability,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| EditableTextState::new(StringStorage::default(), cx));
        let focus_handle = input.focus_handle(cx);
        let input_subscriptions = [
            cx.subscribe(&input, |this, _, _: &TextChanged, cx| {
                this.update_matches(cx)
            }),
            cx.observe(&input, |_, _, cx| cx.notify()),
        ];
        window.focus(&focus_handle, cx);
        let mut palette = Self {
            input,
            focus_handle,
            availability,
            matches: Vec::new(),
            selected: 0,
            _input_subscriptions: input_subscriptions,
        };
        palette.update_matches(cx);
        palette
    }

    pub(super) fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus_handle, cx);
    }

    pub(super) fn set_availability(
        &mut self,
        availability: CommandAvailability,
        cx: &mut Context<Self>,
    ) {
        if self.availability == availability {
            return;
        }
        let selected = self.matches.get(self.selected).map(|item| item.command);
        self.availability = availability;
        self.update_matches(cx);
        if let Some(selected) = selected
            && let Some(index) = self
                .matches
                .iter()
                .position(|item| item.command == selected)
        {
            self.selected = index;
        }
    }

    fn update_matches(&mut self, cx: &mut Context<Self>) {
        let query = self.input.read(cx).as_str().to_owned();
        self.matches = COMMAND_REGISTRY.search(&query, self.availability);
        self.selected = self.selected.min(self.matches.len().saturating_sub(1));
        cx.notify();
    }

    fn move_selection(&mut self, down: bool, cx: &mut Context<Self>) -> bool {
        if self.matches.is_empty() {
            return false;
        }
        self.selected = if down {
            (self.selected + 1) % self.matches.len()
        } else if self.selected == 0 {
            self.matches.len() - 1
        } else {
            self.selected - 1
        };
        cx.notify();
        true
    }

    fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.selected != index {
            self.selected = index;
            cx.notify();
        }
    }

    fn choose(&mut self, command: Command, cx: &mut Context<Self>) {
        cx.emit(CommandPaletteEvent::Run(command));
    }

    fn submit(&mut self, _: &Enter, _: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        if let Some(command) = self.matches.get(self.selected).map(|item| item.command) {
            self.choose(command, cx);
        }
    }

    fn dismiss(&mut self, _: &Escape, _: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        cx.emit(CommandPaletteEvent::Dismiss);
    }

    fn select_previous(&mut self, _: &NavUp, _: &mut Window, cx: &mut Context<Self>) {
        if self.move_selection(false, cx) {
            cx.stop_propagation();
        } else {
            cx.propagate();
        }
    }

    fn select_next(&mut self, _: &NavDown, _: &mut Window, cx: &mut Context<Self>) {
        if self.move_selection(true, cx) {
            cx.stop_propagation();
        } else {
            cx.propagate();
        }
    }
}

impl Focusable for CommandPalette {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for CommandPalette {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = super::theme::palette(window, cx);
        let input = h_stack()
            .items_center()
            .gap(px(metrics::OMNIBOX_GAP))
            .h(px(metrics::COMMAND_PALETTE_INPUT_HEIGHT))
            .px(px(metrics::COMMAND_PALETTE_PADDING))
            .border_b_1()
            .border_color(rgba(palette.menu_border))
            .child(search_icon_sized(
                palette.text_secondary,
                metrics::OMNIBOX_ICON_SIZE,
            ))
            .child(
                themed_text_input(text_input("command-palette-input"), palette)
                    .state(self.input.downgrade())
                    .track_focus(&self.focus_handle)
                    .aria_label("Search browser commands")
                    .placeholder("Search commands")
                    .placeholder_color(rgb(palette.text_secondary))
                    .flex_1()
                    .min_w_0()
                    .whitespace_nowrap()
                    .overflow_x_scroll(),
            );

        let rows: Vec<_> =
            self.matches
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    let command = item.command;
                    let selected = index == self.selected;
                    let highlights: Vec<(std::ops::Range<usize>, HighlightStyle)> = item
                        .name_matches
                        .iter()
                        .cloned()
                        .map(|range| {
                            (
                                range,
                                HighlightStyle {
                                    font_weight: Some(FontWeight::SEMIBOLD),
                                    ..HighlightStyle::default()
                                },
                            )
                        })
                        .collect();
                    let mut row =
                        h_stack()
                            .id(("command-palette-row", index))
                            .role(Role::ListBoxOption)
                            .aria_label(command.name())
                            .aria_selected(selected)
                            .items_center()
                            .gap(px(metrics::MENU_ITEM_GAP))
                            .h(px(metrics::COMMAND_PALETTE_ROW_HEIGHT))
                            .px(px(metrics::COMMAND_PALETTE_ROW_INSET))
                            .rounded(px(metrics::menu_item_radius()))
                            .cursor_pointer()
                            .hover_style(|style| style.bg(rgba(palette.hover_surface)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    this.choose(command, cx);
                                }),
                            )
                            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                                if *hovered {
                                    this.select(index, cx);
                                }
                            }))
                            .child(
                                div().flex_1().min_w_0().truncate().child(
                                    StyledText::new(command.name()).with_highlights(highlights),
                                ),
                            )
                            .children(
                                action_for_command(command)
                                    .and_then(|action| action_kbd(action.as_ref(), palette, cx)),
                            );
                    if selected {
                        row = row.bg(rgba(palette.selected_surface));
                    }
                    row
                })
                .collect();

        let result_list = if self.matches.is_empty() {
            v_stack()
                .id("command-palette-empty")
                .h(px(metrics::COMMAND_PALETTE_ROW_HEIGHT))
                .px(px(metrics::COMMAND_PALETTE_ROW_INSET))
                .justify_center()
                .text_size(px(metrics::MENU_FONT_SIZE))
                .text_color(rgb(palette.text_secondary))
                .child("No matching commands")
                .into_any_element()
        } else {
            v_stack()
                .id("command-palette-results")
                .role(Role::ListBox)
                .aria_label("Browser commands")
                .max_h(px(metrics::COMMAND_PALETTE_MAX_RESULTS_HEIGHT))
                .overflow_y_scroll()
                .elastic_scroll(
                    crate::platform::supports_elastic_scroll() && Settings::get(cx).elastic_scroll,
                )
                .p(px(metrics::COMMAND_PALETTE_LIST_PADDING))
                .children(rows)
                .into_any_element()
        };

        v_stack()
            .id("command-palette")
            .w(px(metrics::COMMAND_PALETTE_WIDTH))
            .max_w_full()
            .max_h(px(metrics::COMMAND_PALETTE_MAX_HEIGHT))
            .rounded(px(metrics::surface_radius()))
            .border_1()
            .border_color(rgba(palette.menu_border))
            .raised(palette)
            .text_size(px(metrics::MENU_FONT_SIZE))
            .text_color(rgb(palette.text_primary))
            .elevated(super::layout::Elevation::High)
            .occlude()
            .role(Role::Dialog)
            .aria_label("Command palette")
            .aria_modal(true)
            .on_mouse_down_out(cx.listener(|_, _, _, cx| cx.emit(CommandPaletteEvent::Dismiss)))
            .owns_clicks()
            .capture_action(cx.listener(Self::submit))
            .capture_action(cx.listener(Self::dismiss))
            .capture_action(cx.listener(Self::select_previous))
            .capture_action(cx.listener(Self::select_next))
            .child(input)
            .child(result_list)
            .child(
                h_stack()
                    .items_center()
                    .justify_between()
                    .gap(px(metrics::COMMAND_PALETTE_FOOTER_GAP))
                    .px(px(metrics::COMMAND_PALETTE_PADDING))
                    .h(px(metrics::COMMAND_PALETTE_FOOTER_HEIGHT))
                    .border_t_1()
                    .border_color(rgba(palette.menu_border))
                    .text_size(px(metrics::KBD_FONT_SIZE))
                    .text_color(rgb(palette.text_secondary))
                    .child("↑ ↓ Navigate")
                    .child(
                        h_stack()
                            .items_center()
                            .gap(px(metrics::KBD_GAP))
                            .child(keycap_label("Enter", palette))
                            .child("Run"),
                    )
                    .child("Esc Close"),
            )
    }
}
