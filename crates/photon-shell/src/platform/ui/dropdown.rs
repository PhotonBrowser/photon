//! A dropdown: a button showing the chosen option, which opens a menu of
//! the options below it.

use std::rc::Rc;
use std::time::Instant;

use gpui::{App, ElementId, MouseButton, Role, Window, deferred, div, prelude::*, px, rgb, rgba};

use super::controls::Choice;
use super::icons::chevron_down_icon;
use super::layout::{h_stack, v_stack};
use super::menu::{MENU_MOTION, menu_checkbox, popover_surface};
use super::motion::Transition;
use super::{metrics, theme::ThemeColors};

/// A button showing the chosen option, which opens a menu of the options
/// below it.
pub(super) fn dropdown(
    id: &'static str,
    label: &'static str,
    options: Vec<Choice>,
    palette: ThemeColors,
) -> Dropdown {
    Dropdown {
        id,
        label,
        options,
        palette,
    }
}

/// Whether the menu is open, and when it last closed, so it can animate
/// away.
#[derive(Clone, Copy, Default)]
struct OpenState {
    open: bool,
    closed_at: Option<Instant>,
}

#[derive(IntoElement)]
pub(super) struct Dropdown {
    id: &'static str,
    label: &'static str,
    options: Vec<Choice>,
    palette: ThemeColors,
}

impl RenderOnce for Dropdown {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self {
            id,
            label,
            options,
            palette,
        } = self;
        let open_state =
            window.use_keyed_state(ElementId::from(id), cx, |_, _| OpenState::default());
        let OpenState { open, closed_at } = *open_state.read(cx);
        // A closing menu stays drawn while it animates away, like any menu.
        let transition = if open {
            Some(Transition::Enter)
        } else if closed_at.is_some_and(|at| at.elapsed() < MENU_MOTION.exit_duration()) {
            window.request_animation_frame();
            Some(Transition::Exit)
        } else {
            None
        };
        let set_open = {
            let open_state = open_state.clone();
            move |open: bool, cx: &mut App| {
                open_state.update(cx, |state, cx| {
                    *state = OpenState {
                        open,
                        closed_at: (!open).then(Instant::now),
                    };
                    cx.notify();
                })
            }
        };
        let chosen = options
            .iter()
            .find(|option| option.chosen)
            .map(|option| option.label.clone())
            .unwrap_or_default();
        let menu = transition.map(|transition| {
            let set_open = Rc::new(set_open.clone());
            let rows = options.into_iter().enumerate().map(|(index, option)| {
                let set_open = set_open.clone();
                let on_choose = option.on_choose;
                menu_checkbox(
                    (id, index),
                    option.label,
                    0,
                    option.chosen,
                    Box::new(move |event, window, cx| {
                        on_choose(event, window, cx);
                        set_open(false, cx);
                    }),
                    palette,
                )
            });
            let close = set_open.clone();
            deferred(
                div()
                    .absolute()
                    .left_0()
                    .top(px(metrics::SEGMENT_HEIGHT + metrics::MENU_ITEM_GAP))
                    .when(open, |menu| {
                        menu.occlude()
                            .on_mouse_down_out(move |_, _, cx| close(false, cx))
                    })
                    .child(popover_surface(
                        label,
                        Role::ListBox,
                        metrics::DROPDOWN_WIDTH,
                        v_stack().children(rows),
                        transition,
                        palette,
                    )),
            )
            .priority(1)
        });
        h_stack()
            .id(id)
            .relative()
            .role(Role::ComboBox)
            .aria_label(label)
            .aria_expanded(open)
            .tab_index(0)
            .focus_visible(|style| style.border_1().border_color(rgb(palette.chosen)))
            .w(px(metrics::DROPDOWN_WIDTH))
            .h(px(metrics::SEGMENT_HEIGHT))
            .px(px(metrics::SEGMENT_HORIZONTAL_PADDING))
            .items_center()
            .justify_between()
            .rounded(px(metrics::CONTROL_RADIUS))
            .bg(rgba(if open {
                palette.selected_surface
            } else {
                palette.surface
            }))
            .hover(|style| style.bg(rgba(palette.selected_surface)))
            .text_size(px(metrics::BUTTON_FONT_SIZE))
            .text_color(rgb(palette.text_primary))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(move |_, _, cx| set_open(!open, cx))
            .child(chosen)
            .child(chevron_down_icon(
                palette.text_secondary,
                metrics::TOOLBAR_ICON_SIZE,
            ))
            .children(menu)
    }
}
