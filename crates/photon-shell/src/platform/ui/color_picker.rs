//! Choosing an accent colour for a space, with a switch that fades the tint
//! into its neighbour as a gradient. The space editor uses it.

use gpui::{App, ElementId, StatefulInteractiveElement, Window, prelude::*, px, rgb, rgba};
use photon_core::WindowColor;
use std::cell::Cell;
use std::rc::Rc;

use super::controls::OwnsClicks;
use super::controls::{Swatch, haptic_switch_toggle, swatches, switch};
use super::layout::{ControlSurface, h_stack, v_stack};
use super::{ClickHandler, metrics, theme::AccentColor, theme::ThemeColors, theme::accent_swatch};

/// Called with the colour and gradient chosen.
pub(super) type ColorChange = Rc<dyn Fn(WindowColor, bool, &mut Window, &mut App)>;

/// The swatches, with the Gradient switch while a colour is chosen.
pub(super) fn color_picker(
    id: &'static str,
    color: WindowColor,
    gradient: bool,
    palette: ThemeColors,
    on_change: ColorChange,
) -> impl IntoElement {
    let options = AccentColor::ALL
        .into_iter()
        .map(|option| {
            let window_color = option.window_color();
            let on_change = on_change.clone();
            Swatch {
                label: option.name().into(),
                color: accent_swatch(option),
                chosen: AccentColor::from_window_color(color) == option,
                on_choose: Box::new(move |_, window, cx| {
                    on_change(window_color, gradient, window, cx)
                }),
            }
        })
        .collect();
    let gradient_switch = (color != WindowColor::System).then(|| {
        let toggle: ClickHandler = {
            let on_change = on_change.clone();
            Box::new(move |_, window, cx| on_change(color, !gradient, window, cx))
        };
        let toggle = Rc::new(toggle);
        let activated = Rc::new(Cell::new(false));
        let row_toggle = haptic_switch_toggle(toggle.clone(), activated.clone());
        let switch_toggle = row_toggle.clone();
        let dragged = Rc::new(Cell::new(false));
        let row_dragged_on_down = dragged.clone();
        let row_dragged_on_click = dragged.clone();
        let row_activated_on_down = activated;
        h_stack()
            .id((ElementId::from(id), "gradient"))
            .items_center()
            .justify_between()
            .w_full()
            .h(px(metrics::SETTINGS_ROW_HEIGHT))
            .px(px(metrics::MENU_ITEM_HORIZONTAL_PADDING))
            .control_surface(palette)
            .text_size(px(metrics::INTERNAL_PAGE_BODY_SIZE))
            .text_color(rgb(palette.text_primary))
            .hover_style(|style| style.bg(rgba(palette.hover_surface)))
            .owns_clicks()
            .on_mouse_down_all(move |event, phase, _, _, _| {
                if phase == gpui::DispatchPhase::Capture && event.button == gpui::MouseButton::Left
                {
                    row_dragged_on_down.set(false);
                    row_activated_on_down.set(false);
                }
            })
            .on_click(move |event, window, cx| {
                if !row_dragged_on_click.replace(false) {
                    row_toggle(event, window, cx);
                }
            })
            .child(
                gpui::div()
                    .id(ElementId::Name(format!("{id}-gradient-label").into()))
                    .aria_hidden()
                    .child("Gradient"),
            )
            .child(switch(
                id,
                gradient,
                palette,
                Some("Gradient"),
                toggle,
                switch_toggle,
                dragged,
            ))
    });
    v_stack()
        .gap(px(metrics::MENU_ITEM_GAP))
        .child(swatches(id, "Accent color", options, palette))
        .children(gradient_switch)
}
