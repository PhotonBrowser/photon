//! The command bar's rows: addresses and searches, open tabs and commands,
//! each with its icon, its text with the typed words in bold, and what
//! choosing it does.

use gpui::{
    AnyElement, App, ImageSource, MouseButton, ObjectFit, Role, StyledText, Window, div, img,
    prelude::*, px, rgb, rgba,
};
use photon_core::{Command, CommandBarResult};
use std::rc::Rc;

use super::super::history::BrowsingHistory;
use super::super::icons::{
    add_icon, close_icon, copy_icon, globe_icon, history_icon, reload_icon, search_icon_sized,
    settings_icon, sidebar_icon, window_icon,
};
use super::super::layout::{h_stack, v_stack};
use super::super::omnibox::{icon_slot, matched_text, row_text, suggestion_icon};
use super::super::{metrics, theme::ThemeColors};

/// The rows, with `selected` highlighted as what Enter chooses. Pointing at
/// a row selects it.
pub(super) fn result_rows(
    results: &[CommandBarResult],
    selected: usize,
    palette: ThemeColors,
    choose: impl Fn(usize, &mut Window, &mut App) + 'static,
    select: impl Fn(usize, &mut Window, &mut App) + 'static,
    cx: &mut App,
) -> gpui::Stateful<gpui::Div> {
    let (choose, select) = (Rc::new(choose), Rc::new(select));
    let rows: Vec<_> = results
        .iter()
        .enumerate()
        .map(|(index, result)| {
            let (choose, select) = (choose.clone(), select.clone());
            let (icon, label, detail) = row_parts(result, palette, cx);
            let row = h_stack()
                .id(("command-bar-row", index))
                .role(Role::ListBoxOption)
                .aria_selected(index == selected)
                .items_center()
                .gap(px(metrics::OMNIBOX_GAP))
                .w_full()
                .h(px(metrics::COMMAND_BAR_ROW_HEIGHT))
                .px(px(
                    metrics::COMMAND_BAR_FIELD_PADDING - metrics::COMMAND_BAR_ROWS_INSET
                ))
                .rounded(px(metrics::MENU_ITEM_RADIUS))
                .text_size(px(metrics::MENU_FONT_SIZE))
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    cx.stop_propagation();
                    choose(index, window, cx);
                })
                .on_hover(move |hovered, window, cx| {
                    if *hovered {
                        select(index, window, cx);
                    }
                })
                .child(icon)
                .child(div().flex_1().min_w_0().truncate().child(label))
                .child(
                    div()
                        .flex_shrink_0()
                        .text_size(px(metrics::COMMAND_BAR_DETAIL_FONT_SIZE))
                        .text_color(rgb(palette.text_secondary))
                        .child(detail),
                );
            if index == selected {
                row.bg(rgba(palette.selected_surface))
            } else {
                row
            }
        })
        .collect();
    v_stack()
        .id("command-bar-results")
        .role(Role::ListBox)
        .aria_label("Results")
        .w_full()
        .border_t_1()
        .border_color(rgba(palette.menu_border))
        .p(px(metrics::COMMAND_BAR_ROWS_INSET))
        .children(rows)
}

/// A row's icon, text and what choosing it does.
fn row_parts(
    result: &CommandBarResult,
    palette: ThemeColors,
    cx: &mut App,
) -> (AnyElement, StyledText, &'static str) {
    match result {
        CommandBarResult::Open(suggestion) => {
            let (text, highlights) = row_text(suggestion, palette);
            (
                suggestion_icon(suggestion, palette, cx),
                StyledText::new(text).with_highlights(highlights),
                "Open in New Tab",
            )
        }
        CommandBarResult::SwitchToTab {
            title,
            url,
            title_matches,
            ..
        } => {
            let (text, highlights) = matched_text(title, title_matches);
            let icon = match BrowsingHistory::favicon(url, cx) {
                Some(favicon) => img(ImageSource::Render(favicon.image))
                    .size(px(metrics::OMNIBOX_SUGGESTION_ICON_SIZE))
                    .object_fit(ObjectFit::Contain)
                    .into_any_element(),
                None => globe_icon(palette.text_secondary, metrics::OMNIBOX_ICON_SIZE)
                    .into_any_element(),
            };
            (
                icon_slot(icon),
                StyledText::new(text).with_highlights(highlights),
                "Switch to Tab",
            )
        }
        CommandBarResult::Run {
            command,
            name_matches,
        } => {
            let (text, highlights) = matched_text(command.name(), name_matches);
            (
                icon_slot(command_icon(*command, palette)),
                StyledText::new(text).with_highlights(highlights),
                "Command",
            )
        }
    }
}

fn command_icon(command: Command, palette: ThemeColors) -> AnyElement {
    let (color, size) = (palette.text_secondary, metrics::OMNIBOX_ICON_SIZE);
    match command {
        Command::NewTab => add_icon(color, size).into_any_element(),
        Command::NewWindow => window_icon(color, size).into_any_element(),
        Command::ReopenClosedTab => history_icon(color, size).into_any_element(),
        Command::CloseTab => close_icon(color, size).into_any_element(),
        Command::ReloadPage => reload_icon(color, size).into_any_element(),
        Command::FindInPage => search_icon_sized(color, size).into_any_element(),
        Command::CopyAddress => copy_icon(color, size).into_any_element(),
        Command::ToggleSidebar => sidebar_icon(color, size).into_any_element(),
        Command::OpenSettings => settings_icon(color, size).into_any_element(),
    }
}
