//! A centered modal surface that dims and blocks what it covers.

use gpui::{App, Div, ElementId, FontWeight, SharedString, div, prelude::*, px, rgb, rgba};

use super::controls::OwnsClicks;
use super::layout::v_stack;
use super::layout::{Elevated, Elevation, Raised};
use super::motion::{AnimateIn, Entrance, Transition, distance};
use super::settings::Settings;
use super::{metrics, theme::ThemeColors};

/// The modal's panel: a raised surface sized for short content. Callers give
/// it an id, role, label, focus and children before passing it to [`modal`].
pub(super) fn modal_panel(palette: ThemeColors) -> Div {
    v_stack()
        .w(px(metrics::MODAL_WIDTH))
        .max_w_full()
        .gap(px(metrics::MODAL_GAP))
        .p(px(metrics::MODAL_PADDING))
        .rounded(px(metrics::surface_radius()))
        .border_1()
        .border_color(rgba(palette.menu_border))
        .raised(palette)
        .text_size(px(metrics::DIALOG_TEXT_SIZE))
        .text_color(rgb(palette.text_primary))
        .elevated(Elevation::High)
}

/// A dialog's title, the same in every dialog.
pub(super) fn dialog_title(title: impl Into<SharedString>, palette: ThemeColors) -> Div {
    div()
        .text_size(px(metrics::DIALOG_TITLE_SIZE))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(palette.text_primary))
        .child(title.into())
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
    cx: &App,
) -> impl IntoElement
where
    E: Styled + IntoElement + 'static,
{
    let panel = panel.animate(
        ElementId::Name(format!("{id}-panel").into()),
        PANEL_MOTION,
        transition,
        &Settings::motion(cx),
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
        modal = modal.occlude().owns_clicks();
    }
    modal.child(panel).animate(
        ElementId::Name(format!("{id}-backdrop").into()),
        MODAL_MOTION,
        transition,
        &Settings::motion(cx),
    )
}
