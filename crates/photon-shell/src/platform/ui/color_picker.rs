//! Choosing a window colour: the colours as swatches, and a switch that
//! fades a colour into its neighbour. Settings → Appearance and the space
//! dialog both use it.

use gpui::{App, ElementId, MouseButton, Role, Window, prelude::*, px, rgb, rgba};
use photon_core::WindowColor;
use std::rc::Rc;

use super::controls::{Swatch, swatches, switch, toggled};
use super::layout::{h_stack, v_stack};
use super::{metrics, theme::ThemeColors, theme::window_color_swatch};

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
        h_stack()
            .id((ElementId::from(id), "gradient"))
            .role(Role::Switch)
            .aria_label("Gradient")
            .aria_toggled(toggled(gradient))
            .tab_index(0)
            .focus_visible(|style| style.border_1().border_color(rgb(palette.chosen)))
            .items_center()
            .justify_between()
            .w_full()
            .h(px(metrics::SETTINGS_ROW_HEIGHT))
            .px(px(metrics::MENU_ITEM_HORIZONTAL_PADDING))
            .rounded(px(metrics::CONTROL_RADIUS))
            .bg(rgba(palette.surface))
            .text_size(px(metrics::INTERNAL_PAGE_BODY_SIZE))
            .text_color(rgb(palette.text_primary))
            .hover(|style| style.bg(rgba(palette.hover_surface)))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(move |_, window, cx| on_change(color, !gradient, window, cx))
            .child("Gradient")
            .child(switch(id, gradient, palette))
    });
    v_stack()
        .gap(px(metrics::MENU_ITEM_GAP))
        .child(swatches(id, "Window colour", options, palette))
        .children(gradient_switch)
}
