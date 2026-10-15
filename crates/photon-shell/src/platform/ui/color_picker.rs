//! Choosing a window colour: the colours as swatches, and a switch that
//! fades a colour into its neighbour. Settings → Appearance and the space
//! dialog both use it.

use gpui::{App, ElementId, StatefulInteractiveElement, Window, prelude::*, px, rgb, rgba};
use photon_core::WindowColor;
use std::cell::Cell;
use std::rc::Rc;

use super::controls::OwnsClicks;
use super::controls::{Swatch, swatches, switch};
use super::layout::{ControlSurface, h_stack, v_stack};
use super::{ClickHandler, metrics, theme::ThemeColors, theme::window_color_swatch};

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
    let options = WindowColor::ALL
        .into_iter()
        .map(|option| {
            let on_change = on_change.clone();
            Swatch {
                label: option.name().into(),
                color: window_color_swatch(option, palette),
                chosen: option == color,
                on_choose: Box::new(move |_, window, cx| on_change(option, gradient, window, cx)),
            }
        })
        .collect();
    let gradient_switch = (color != WindowColor::System).then(|| {
        let toggle: ClickHandler = {
            let on_change = on_change.clone();
            Box::new(move |_, window, cx| on_change(color, !gradient, window, cx))
        };
        let toggle = Rc::new(toggle);
        let row_toggle = toggle.clone();
        let dragged = Rc::new(Cell::new(false));
        let row_dragged_on_down = dragged.clone();
        let row_dragged_on_click = dragged.clone();
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
            .hover(|style| style.bg(rgba(palette.hover_surface)))
            .owns_clicks()
            .on_mouse_down_all(move |event, phase, _, _, _| {
                if phase == gpui::DispatchPhase::Capture && event.button == gpui::MouseButton::Left
                {
                    row_dragged_on_down.set(false);
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
                dragged,
            ))
    });
    v_stack()
        .gap(px(metrics::MENU_ITEM_GAP))
        .child(swatches(id, "Window colour", options, palette))
        .children(gradient_switch)
}
