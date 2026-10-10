//! Suggestion rows: visited pages, past searches, the typed address and a
//! search, drawn in the panel the omnibox opens into while typing.

use gpui::{
    AnyElement, App, FontWeight, HighlightStyle, ImageSource, MouseButton, ObjectFit, Role,
    SharedString, StyledText, Window, div, img, prelude::*, px, rgb, rgb_to_hsla,
};
use photon_core::{Suggestion, SuggestionKind};
use std::ops::Range;
use std::rc::Rc;

use super::super::button::icon_button;
use super::super::history::BrowsingHistory;
use super::super::icons::{close_icon, globe_icon, history_icon, search_icon_sized};
use super::super::layout::{h_stack, v_stack};
use super::super::{metrics, theme::ThemeColors};

/// Called with the index of a suggestion.
pub(super) type SuggestionHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// What the rows do when used.
pub(super) struct SuggestionActions {
    /// Opens the suggestion.
    pub(super) choose: SuggestionHandler,
    /// Forgets a remembered page or search.
    pub(super) remove: SuggestionHandler,
    /// Called with the row under the pointer, or `None` when it leaves.
    pub(super) hover: Rc<dyn Fn(Option<usize>, &mut Window, &mut App)>,
}

/// The rows, with `selected` highlighted as what Enter opens. The selected
/// or hovered row offers to forget what it remembers.
pub(super) fn suggestion_rows(
    suggestions: &[Suggestion],
    selected: usize,
    hovered: Option<usize>,
    palette: ThemeColors,
    actions: &SuggestionActions,
    cx: &mut App,
) -> gpui::Stateful<gpui::Div> {
    let rows: Vec<_> = suggestions
        .iter()
        .enumerate()
        .map(|(index, suggestion)| {
            let choose = actions.choose.clone();
            let hover = actions.hover.clone();
            let mut row = suggestion_row(suggestion, index, index == selected, palette, cx)
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    cx.stop_propagation();
                    choose(index, window, cx);
                })
                .on_hover(move |hovered, window, cx| hover(hovered.then_some(index), window, cx));
            let offers_removal = index == selected || hovered == Some(index);
            if suggestion.kind.is_remembered() && offers_removal {
                let remove = actions.remove.clone();
                row = row.child(icon_button(
                    "omnibox-suggestion-remove",
                    "Remove suggestion",
                    true,
                    metrics::CHIP_CLOSE_SIZE,
                    close_icon(palette.text_secondary, metrics::CHIP_CLOSE_ICON_SIZE),
                    palette,
                    Box::new(move |_, window, cx| {
                        cx.stop_propagation();
                        remove(index, window, cx);
                    }),
                ));
            }
            row
        })
        .collect();
    v_stack()
        .id("omnibox-suggestions")
        .role(Role::ListBox)
        .aria_label("Suggestions")
        .w_full()
        .px(px(metrics::OMNIBOX_SUGGESTIONS_INSET))
        .pb(px(metrics::OMNIBOX_SUGGESTIONS_INSET))
        .children(rows)
}

/// The icon a suggestion shows: the page's icon when known.
pub(super) fn suggestion_icon(
    suggestion: &Suggestion,
    palette: ThemeColors,
    cx: &mut App,
) -> AnyElement {
    let color = palette.text_secondary;
    let size = metrics::OMNIBOX_ICON_SIZE;
    let icon = match &suggestion.kind {
        SuggestionKind::Search { .. } => search_icon_sized(color, size).into_any_element(),
        SuggestionKind::PastSearch => history_icon(color, size).into_any_element(),
        SuggestionKind::Page { .. } | SuggestionKind::Address => {
            match BrowsingHistory::favicon(&suggestion.url, cx) {
                Some(favicon) => img(ImageSource::Render(favicon.image))
                    .size(px(metrics::OMNIBOX_SUGGESTION_ICON_SIZE))
                    .object_fit(ObjectFit::Contain)
                    .into_any_element(),
                None => globe_icon(color, size).into_any_element(),
            }
        }
    };
    icon_slot(icon)
}

/// Centers an icon in the space every row and the field give their icons,
/// so text lines up whatever the icon.
pub(super) fn icon_slot(icon: impl IntoElement) -> AnyElement {
    h_stack()
        .flex_shrink_0()
        .size(px(metrics::OMNIBOX_SUGGESTION_ICON_SIZE))
        .items_center()
        .justify_center()
        .child(icon)
        .into_any_element()
}

fn suggestion_row(
    suggestion: &Suggestion,
    index: usize,
    selected: bool,
    palette: ThemeColors,
    cx: &mut App,
) -> gpui::Stateful<gpui::Div> {
    let (label, highlights) = row_text(suggestion, palette);
    let mut row = h_stack()
        .id(("omnibox-suggestion", index))
        .role(Role::ListBoxOption)
        .aria_label(label.clone())
        .aria_selected(selected)
        .items_center()
        .gap(px(metrics::OMNIBOX_GAP))
        .w_full()
        .h(px(metrics::OMNIBOX_SUGGESTION_HEIGHT))
        .px(px(
            metrics::OMNIBOX_HORIZONTAL_PADDING - metrics::OMNIBOX_SUGGESTIONS_INSET
        ))
        .rounded(px(metrics::MENU_ITEM_RADIUS))
        .hover(|style| style.bg(gpui::rgba(palette.hover_surface)))
        .child(suggestion_icon(suggestion, palette, cx))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .child(StyledText::new(label).with_highlights(highlights)),
        );
    if selected {
        row = row.bg(gpui::rgba(palette.selected_surface));
    }
    row
}

/// A row's text, such as "Discord - discord.com/channels/me" or
/// "rust - Google Search", with the typed words in bold and addresses tinted.
fn row_text(
    suggestion: &Suggestion,
    palette: ThemeColors,
) -> (SharedString, Vec<(Range<usize>, HighlightStyle)>) {
    let address = HighlightStyle {
        color: Some(rgb_to_hsla(rgb(palette.suggestion_address))),
        ..HighlightStyle::default()
    };
    let secondary = HighlightStyle {
        color: Some(rgb_to_hsla(rgb(palette.text_secondary))),
        ..HighlightStyle::default()
    };
    let mut text = String::new();
    let mut highlights = Vec::new();
    match &suggestion.kind {
        SuggestionKind::Page {
            title,
            title_matches,
        } if !title.is_empty() => {
            push(&mut text, &mut highlights, title, title_matches, None);
            push(&mut text, &mut highlights, " - ", &[], None);
            let text_matches = &suggestion.text_matches;
            push(
                &mut text,
                &mut highlights,
                &suggestion.text,
                text_matches,
                Some(address),
            );
        }
        SuggestionKind::Page { .. } | SuggestionKind::Address => {
            let text_matches = &suggestion.text_matches;
            push(
                &mut text,
                &mut highlights,
                &suggestion.text,
                text_matches,
                Some(address),
            );
        }
        SuggestionKind::Search { engine } => {
            push(&mut text, &mut highlights, &suggestion.text, &[], None);
            let detail = format!(" - {engine} Search");
            push(&mut text, &mut highlights, &detail, &[], Some(secondary));
        }
        SuggestionKind::PastSearch => {
            // The rest of a past search stands out from what was typed.
            let typed = suggestion.text_matches.first().map_or(0, |range| range.end);
            let rest = [typed..suggestion.text.len()];
            push(&mut text, &mut highlights, &suggestion.text, &rest, None);
        }
    }
    (text.into(), highlights)
}

/// Appends `part` to `text`, styled with `base` and with `matches` in bold.
fn push(
    text: &mut String,
    highlights: &mut Vec<(Range<usize>, HighlightStyle)>,
    part: &str,
    matches: &[Range<usize>],
    base: Option<HighlightStyle>,
) {
    let offset = text.len();
    text.push_str(part);
    let bold = HighlightStyle {
        font_weight: Some(FontWeight::BOLD),
        ..base.unwrap_or_default()
    };
    let mut matches: Vec<Range<usize>> = matches
        .iter()
        .filter(|range| range.end <= part.len() && range.start < range.end)
        .cloned()
        .collect();
    matches.sort_by_key(|range| range.start);
    let mut position = 0;
    for range in matches {
        let start = range.start.max(position);
        if start >= range.end {
            continue;
        }
        if let Some(base) = base
            && position < start
        {
            highlights.push((offset + position..offset + start, base));
        }
        highlights.push((offset + start..offset + range.end, bold));
        position = range.end;
    }
    if let Some(base) = base
        && position < part.len()
    {
        highlights.push((offset + position..offset + part.len(), base));
    }
}
