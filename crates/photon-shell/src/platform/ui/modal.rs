//! A centered modal surface that dims and blocks what it covers.

use gpui::{
    Animation, AnimationExt, Div, ElementId, MouseButton, div, ease_out_quint, prelude::*, px, rgb,
    rgba,
};
use std::time::Duration;

use super::layout::v_stack;
use super::{metrics, theme::ThemeColors};

/// How long a modal takes to fade in.
const APPEAR_DURATION: Duration = Duration::from_millis(220);

/// The modal's panel: a raised surface sized for short content. Callers give
/// it an id, role, label, focus and children before passing it to [`modal`].
pub(super) fn modal_panel(palette: ThemeColors) -> Div {
    v_stack()
        .w(px(metrics::MODAL_WIDTH))
        .max_w_full()
        .gap(px(metrics::MODAL_GAP))
        .p(px(metrics::MODAL_PADDING))
        .rounded(px(metrics::SURFACE_RADIUS))
        .border_1()
        .border_color(rgba(palette.menu_border))
        .bg(rgba(palette.menu_surface))
        .text_color(rgb(palette.text_primary))
        .shadow_lg()
}

/// Centers `panel` over its positioned container, dimming the container and
/// keeping pointer input from reaching what lies beneath. The modal fades in
/// while the panel settles into place; with reduced motion it appears at once.
pub(super) fn modal<E>(id: &'static str, palette: ThemeColors, panel: E) -> impl IntoElement
where
    E: Styled + IntoElement + 'static,
{
    let panel = panel.with_animation(
        ElementId::Name(format!("{id}-panel").into()),
        Animation::new(APPEAR_DURATION).with_easing(ease_out_quint()),
        |panel, delta| panel.mt(px((1.0 - delta) * metrics::MODAL_APPEAR_RISE)),
    );
    div()
        .id(id)
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .p(px(metrics::MODAL_SURFACE_INSET))
        .bg(rgba(palette.modal_backdrop))
        .occlude()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
        .child(panel)
        .with_animation(
            ElementId::Name(format!("{id}-backdrop").into()),
            Animation::new(APPEAR_DURATION),
            |backdrop, delta| backdrop.opacity(delta),
        )
}
