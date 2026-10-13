//! A dropdown: a button showing the chosen option, which opens a menu of
//! the options below it.

use std::rc::Rc;
use std::time::Instant;

use gpui::{
    App, ClickEvent, ElementId, KeyDownEvent, Role, Window, deferred, div, prelude::*, px, rgb,
    rgba,
};

use super::controls::{Choice, FocusRing, OwnsClicks, Pressable};
use super::icons::chevron_down_icon;
use super::layout::{ControlSurface, h_stack, v_stack};
use super::menu::{MENU_MOTION, menu_option, popover_surface};
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

type ChooseHandler = dyn Fn(&ClickEvent, &mut Window, &mut App);
/// Opens or closes the menu.
type SetOpen = Rc<dyn Fn(bool, &mut Window, &mut App)>;

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
        let animations = super::settings::Settings::get(cx).animations;
        let open_state =
            window.use_keyed_state(ElementId::from(id), cx, |_, _| OpenState::default());
        let focus = window
            .use_keyed_state(
                ElementId::Name(format!("{id}-focus").into()),
                cx,
                |_, cx| cx.focus_handle(),
            )
            .read(cx)
            .clone();
        let OpenState { open, closed_at } = *open_state.read(cx);
        // A closing menu stays drawn while it animates away, like any menu.
        let transition = if open {
            Some(Transition::Enter)
        } else if closed_at.is_some_and(|at| at.elapsed() < MENU_MOTION.exit_duration(&animations))
        {
            window.request_animation_frame();
            Some(Transition::Exit)
        } else {
            None
        };
        // Closing returns focus to the control, as the menu's rows go away.
        let set_open: SetOpen = {
            let open_state = open_state.clone();
            let focus = focus.clone();
            Rc::new(move |open: bool, window: &mut Window, cx: &mut App| {
                open_state.update(cx, |state, cx| {
                    *state = OpenState {
                        open,
                        closed_at: (!open).then(Instant::now),
                    };
                    cx.notify();
                });
                if !open {
                    window.focus(&focus, cx);
                }
            })
        };
        let chosen_index = options.iter().position(|option| option.chosen);
        let chosen = chosen_index
            .map(|index| options[index].label.clone())
            .unwrap_or_default();
        // Each option's action is shared by its row and the control's arrow keys.
        let (labels, choose): (Vec<_>, Vec<Rc<ChooseHandler>>) = options
            .into_iter()
            .map(|option| ((option.label, option.chosen), Rc::from(option.on_choose)))
            .unzip();
        let menu = transition.map(|transition| {
            let rows = labels
                .into_iter()
                .zip(choose.iter().cloned())
                .enumerate()
                .map(|(index, ((label, chosen), on_choose))| {
                    let set_open = set_open.clone();
                    menu_option(
                        (id, index),
                        label,
                        chosen,
                        Box::new(move |event, window, cx| {
                            on_choose(event, window, cx);
                            set_open(false, window, cx);
                        }),
                        palette,
                    )
                });
            let close = set_open.clone();
            let close_on_escape = set_open.clone();
            deferred(
                div()
                    .absolute()
                    .left_0()
                    .top(px(metrics::BUTTON_HEIGHT + metrics::MENU_ITEM_GAP))
                    .when(open, |menu| {
                        menu.occlude()
                            .on_mouse_down_out(move |_, window, cx| close(false, window, cx))
                            // Up and Down move between the options.
                            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                                if event.keystroke.modifiers.modified() {
                                    return;
                                }
                                match event.keystroke.key.as_str() {
                                    "escape" => close_on_escape(false, window, cx),
                                    "down" => window.focus_next(cx),
                                    "up" => window.focus_prev(cx),
                                    _ => return,
                                }
                                cx.stop_propagation();
                            })
                    })
                    .child(popover_surface(
                        label,
                        Role::ListBox,
                        metrics::DROPDOWN_WIDTH,
                        v_stack().children(rows),
                        transition,
                        palette,
                        cx,
                    )),
            )
            .priority(1)
        });
        let toggle = set_open.clone();
        let close = set_open;
        h_stack()
            .id(id)
            .relative()
            .role(Role::ComboBox)
            .aria_label(label)
            .aria_expanded(open)
            .track_focus(&focus)
            .tab_index(0)
            .focus_ring(palette)
            .w(px(metrics::DROPDOWN_WIDTH))
            .h(px(metrics::BUTTON_HEIGHT))
            .px(px(metrics::SEGMENT_HORIZONTAL_PADDING))
            .items_center()
            .justify_between()
            .control_surface(palette)
            .when(open, |control| control.bg(rgba(palette.selected_surface)))
            .hover(|style| style.bg(rgba(palette.surface_hover)))
            .text_size(px(metrics::BUTTON_FONT_SIZE))
            .text_color(rgb(palette.text_primary))
            .owns_clicks()
            .pressable()
            .on_click(move |_, window, cx| toggle(!open, window, cx))
            // Closed, Up and Down step through the options without opening
            // the menu, as a native pop-up button does.
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                if event.keystroke.modifiers.modified() {
                    return;
                }
                let step: isize = match event.keystroke.key.as_str() {
                    "escape" if open => {
                        close(false, window, cx);
                        cx.stop_propagation();
                        return;
                    }
                    "down" if !open => 1,
                    "up" if !open => -1,
                    _ => return,
                };
                cx.stop_propagation();
                let next = chosen_index.map_or(0, |index| {
                    (index as isize + step).clamp(0, choose.len() as isize - 1) as usize
                });
                if Some(next) != chosen_index
                    && let Some(on_choose) = choose.get(next)
                {
                    on_choose(&ClickEvent::default(), window, cx);
                }
            })
            .child(div().flex_1().min_w_0().truncate().child(chosen))
            .child(chevron_down_icon(
                palette.text_secondary,
                metrics::TOOLBAR_ICON_SIZE,
            ))
            .children(menu)
    }
}
