//! A centered modal surface that dims and blocks what it covers.

use gpui::{Div, ElementId, MouseButton, div, prelude::*, px, rgb, rgba};

use super::layout::v_stack;
use super::layout::{Elevated, Elevation};
use super::motion::{AnimateIn, Entrance, Transition, distance};
use super::{metrics, theme::ThemeColors};

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
        .elevated(Elevation::High)
}

/// The backdrop's fade, which also fades the panel.
pub(super) const MODAL_MOTION: Entrance = Entrance::new().fade_from(0.0);
/// The panel only rises as the backdrop fades it in.
const PANEL_MOTION: Entrance = Entrance::new().slide_up(distance::SHIFT);

/// Centers `panel` over its positioned container, dimming the container and
/// keeping pointer input from reaching what lies beneath. The modal fades in
/// while the panel settles into place, and leaves the same way; with reduced
/// motion it appears at once.
pub(super) fn modal<E>(
    id: &'static str,
    transition: Transition,
    palette: ThemeColors,
    panel: E,
) -> impl IntoElement
where
    E: Styled + IntoElement + 'static,
{
    let panel = panel.animate(
        ElementId::Name(format!("{id}-panel").into()),
        PANEL_MOTION,
        transition,
    );
    let mut modal = div()
        .id(id)
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .p(px(metrics::MODAL_SURFACE_INSET))
        .bg(rgba(palette.modal_backdrop));
    // A closing modal no longer blocks what it covers.
    if transition == Transition::Enter {
        modal = modal
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation());
    }
    modal.child(panel).animate(
        ElementId::Name(format!("{id}-backdrop").into()),
        MODAL_MOTION,
        transition,
    )
}
