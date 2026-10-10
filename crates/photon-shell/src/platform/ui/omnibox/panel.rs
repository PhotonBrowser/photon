//! The field's content, and the panel it opens into while typing.

use gpui::{AnyElement, Context, Focusable, MouseButton, deferred, prelude::*, px, rgb, rgba};
use gpui_elements::editable_text::text_input;
use std::rc::Rc;

use super::super::controls::themed_text_input;
use super::super::icons::search_icon_sized;
use super::super::layout::{Elevated, Elevation};
use super::super::layout::{h_stack, v_stack};
use super::super::motion::{AnimateIn, Entrance};
use super::super::{metrics, theme::ThemeColors};
use super::Omnibox;
use super::rows::{SuggestionActions, icon_slot, suggestion_icon, suggestion_rows};

impl Omnibox {
    /// The icon and text field, laid out identically whether the field is
    /// closed or open, so the text does not move as it opens.
    pub(super) fn field_content(&self, palette: ThemeColors, cx: &mut Context<Self>) -> gpui::Div {
        let input_focus = self.input.focus_handle(cx).tab_index(3).tab_stop(true);
        h_stack()
            .items_center()
            .gap(px(metrics::OMNIBOX_GAP))
            .w_full()
            .flex_shrink_0()
            .h(px(
                metrics::OMNIBOX_HEIGHT - 2.0 * metrics::OMNIBOX_BORDER_WIDTH
            ))
            .px(px(metrics::OMNIBOX_HORIZONTAL_PADDING))
            .child(self.field_icon(palette, cx))
            .child(
                themed_text_input(text_input("titlebar-omnibox-input"), palette)
                    .state(self.input.downgrade())
                    .track_focus(&input_focus)
                    .placeholder("Search or enter address")
                    .placeholder_color(rgb(palette.text_primary))
                    .flex_1()
                    .min_w_0()
                    .whitespace_nowrap()
                    .overflow_x_scroll(),
            )
    }

    /// The highlighted suggestion's icon while suggesting, otherwise a search icon.
    fn field_icon(&self, palette: ThemeColors, cx: &mut Context<Self>) -> AnyElement {
        match self.suggestions.get(self.selected) {
            Some(suggestion) => suggestion_icon(suggestion, palette, cx),
            None => icon_slot(search_icon_sized(
                palette.text_primary,
                metrics::OMNIBOX_ICON_SIZE,
            )),
        }
    }

    /// The field opened into a panel over the page: its content in the same
    /// place, with the suggestions under it.
    pub(super) fn open_panel(
        &self,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let rows = suggestion_rows(
            &self.suggestions,
            self.selected,
            self.hovered,
            palette,
            &self.suggestion_actions(cx),
            cx,
        );
        // Line up with the closed field, whose border the panel replaces.
        let border = px(-metrics::OMNIBOX_BORDER_WIDTH);
        deferred(
            v_stack()
                .id("omnibox-panel")
                .absolute()
                .top(border)
                .left(border)
                .right(border)
                // A narrow field opens into a panel that reaches over the page.
                .min_w(px(metrics::OMNIBOX_PANEL_MIN_WIDTH))
                .rounded(px(metrics::OMNIBOX_RADIUS))
                .border_1()
                .border_color(rgba(palette.menu_border))
                .bg(rgba(palette.menu_surface))
                .elevated(Elevation::High)
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(self.field_content(palette, cx))
                .child(rows.animate_in("omnibox-suggestions-appear", Entrance::fade())),
        )
        .priority(1)
    }

    fn suggestion_actions(&self, cx: &mut Context<Self>) -> SuggestionActions {
        let this = cx.entity().downgrade();
        let (choose, remove, hover) = (this.clone(), this.clone(), this);
        SuggestionActions {
            choose: Rc::new(move |index, window, cx| {
                choose
                    .update(cx, |this, cx| this.choose(index, window, cx))
                    .ok();
            }),
            remove: Rc::new(move |index, _, cx| {
                remove.update(cx, |this, cx| this.remove(index, cx)).ok();
            }),
            hover: Rc::new(move |index, _, cx| {
                hover.update(cx, |this, cx| this.hover(index, cx)).ok();
            }),
        }
    }
}
