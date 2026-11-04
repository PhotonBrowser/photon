//! The empty split pane's choices of existing tabs.

use gpui::{AnyElement, ElementId, FontWeight, Role, SharedString, div, prelude::*, px, rgb, rgba};

use super::super::super::controls::FocusRing;
use super::super::super::controls::{OwnsClicks, Pressable};
use super::super::super::layout::{h_stack, secondary_text, v_stack};
use super::super::super::motion::{AnimateIn, Entrance, MotionConfig, Speed, SurfaceFade};
use super::super::super::{metrics, theme::ThemeColors};
use super::SplitChoice;

/// How each card rises in, one after another.
const CARD_ENTRANCE: Entrance = Entrance::new()
    .fade_from(0.0)
    .slide_up(metrics::SPLIT_CHOOSER_CARD_RISE)
    .scale_from(0.96)
    .speed(Speed::Standard);

pub(super) fn view(
    choices: Vec<SplitChoice>,
    palette: ThemeColors,
    animations: &MotionConfig,
) -> AnyElement {
    let has_choices = !choices.is_empty();
    let max_grid_width = metrics::SPLIT_CHOOSER_MAX_COLUMNS as f32
        * metrics::SPLIT_CHOOSER_CARD_WIDTH
        + (metrics::SPLIT_CHOOSER_MAX_COLUMNS as f32 - 1.0) * metrics::SPLIT_CHOOSER_GAP;
    let heading = div()
        .text_size(px(metrics::INTERNAL_PAGE_SECTION_SIZE))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(palette.text_primary))
        .child("Choose a tab for this pane");
    let grid = h_stack()
        .id("split-view-tab-choices")
        .role(Role::List)
        .aria_label("Open tabs")
        .w_full()
        .max_w(px(max_grid_width))
        .flex_wrap()
        .justify_center()
        .gap(px(metrics::SPLIT_CHOOSER_GAP))
        .children(choices.into_iter().enumerate().map(|(position, choice)| {
            let id = format!("split-view-choice-{}-entrance", choice.id);
            let delay = std::time::Duration::from_millis(
                metrics::SPLIT_CHOOSER_CARD_STAGGER_MS
                    * position.min(metrics::SPLIT_CHOOSER_STAGGERED_CARDS) as u64,
            );
            div().child(choice_card(choice, palette)).animate_in(
                id,
                CARD_ENTRANCE.delay(delay),
                animations,
            )
        }));
    let content = v_stack()
        .id("split-view-picker-content")
        .w_full()
        .min_h_full()
        .items_center()
        .justify_center()
        .gap(px(metrics::INTERNAL_PAGE_SECTION_GAP))
        .p(px(metrics::SPLIT_CHOOSER_PADDING))
        .child(
            v_stack()
                .items_center()
                .gap(px(metrics::FORM_FIELD_GAP))
                .child(heading)
                .child(secondary_text(
                    if has_choices {
                        "Pick an open tab, or type an address above."
                    } else {
                        "Type an address above to open a page here."
                    },
                    palette,
                )),
        )
        .when(has_choices, |content| content.child(grid));
    div()
        .id("split-view-tab-picker")
        .size_full()
        .overflow_y_scroll()
        .child(content)
        .into_any_element()
}

fn choice_card(choice: SplitChoice, palette: ThemeColors) -> AnyElement {
    let accessible_name = format!("Open {} in split view", choice.title);
    h_stack()
        .id(ElementId::Name(
            format!("split-view-choice-{}", choice.id).into(),
        ))
        .role(Role::Button)
        .aria_label(SharedString::from(accessible_name))
        .aria_description(choice.address.clone())
        .tab_index(0)
        .focus_ring(palette)
        .items_center()
        .gap(px(metrics::SPLIT_CHOOSER_GAP))
        .w(px(metrics::SPLIT_CHOOSER_CARD_WIDTH))
        .h(px(metrics::SPLIT_CHOOSER_CARD_HEIGHT))
        .px(px(metrics::SPLIT_CHOOSER_CARD_PADDING))
        .rounded(px(metrics::surface_radius()))
        .border_1()
        .border_color(rgba(palette.menu_border))
        .bg(rgba(palette.surface))
        .hover_style(|style| style.bg(rgba(palette.surface_hover)))
        .surface_fade()
        .owns_clicks()
        .pressable()
        .on_click(choice.on_select)
        .child(choice.icon)
        .child(
            v_stack()
                .flex_1()
                .min_w_0()
                .gap(px(metrics::SPLIT_CHOOSER_TEXT_GAP))
                .child(
                    div()
                        .w_full()
                        .truncate()
                        .text_size(px(metrics::TAB_FONT_SIZE))
                        .text_color(rgb(palette.text_primary))
                        .child(choice.title),
                )
                .child(
                    div()
                        .w_full()
                        .truncate()
                        .text_size(px(metrics::MENU_FONT_SIZE))
                        .text_color(rgb(palette.text_secondary))
                        .child(choice.address),
                ),
        )
        .into_any_element()
}
