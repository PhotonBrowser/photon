//! Text buttons for alerts and dialogs.

use gpui::{Div, ElementId, Role, SharedString, Stateful, prelude::*, px, rgb, rgba};

use super::ClickHandler;
use super::controls::{FocusRing, OwnsClicks};
use super::icons::more_icon;
use super::layout::{ControlSurface, h_stack};
use super::motion::{icon_pressable, pressable};
use super::{metrics, theme::ThemeColors};
use gpui_motion::PressScaleElement;

/// A button's size.
#[derive(Clone, Copy)]
pub(super) enum ButtonSize {
    /// For dialogs and alerts.
    Regular,
    /// For compact surfaces such as status chips.
    Small,
}

/// What a labelled button does, which sets how it looks.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum ButtonKind {
    /// The default action, filled with the accent.
    Primary,
    /// Any other action, plain until hovered.
    Secondary,
    /// An action that deletes or resets, in the error colour. It is never
    /// the default, so Enter does not run it.
    Destructive,
}

/// A labelled button. A disabled one is dimmed, ignores clicks and is left
/// out of the tab order. Callers can name it more fully for accessibility
/// with `aria_label`, as when several rows share a label.
pub(super) fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    kind: ButtonKind,
    enabled: bool,
    size: ButtonSize,
    palette: ThemeColors,
    on_click: ClickHandler,
) -> PressScaleElement<Stateful<Div>> {
    let label = label.into();
    let (height, min_width, padding, font_size, radius) = match size {
        ButtonSize::Regular => (
            metrics::BUTTON_HEIGHT,
            metrics::BUTTON_MIN_WIDTH,
            metrics::BUTTON_HORIZONTAL_PADDING,
            metrics::BUTTON_FONT_SIZE,
            metrics::control_radius(),
        ),
        ButtonSize::Small => (
            metrics::SMALL_BUTTON_HEIGHT,
            0.0,
            metrics::SMALL_BUTTON_HORIZONTAL_PADDING,
            metrics::SMALL_BUTTON_FONT_SIZE,
            metrics::small_button_radius(),
        ),
    };
    let primary = kind == ButtonKind::Primary;
    let button = h_stack()
        .id(id)
        .role(Role::Button)
        .aria_label(label.clone())
        .aria_disabled(!enabled)
        .focus_visible_style(|style| {
            style.border_color(rgb(if primary {
                palette.on_chosen
            } else {
                palette.chosen
            }))
        })
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .h(px(height))
        .min_w(px(min_width))
        .px(px(padding))
        .control_surface(palette)
        .rounded(px(radius))
        .text_size(px(font_size))
        .child(label);
    let button = match kind {
        ButtonKind::Primary => button
            .bg(rgb(palette.chosen))
            .border_color(rgb(palette.chosen))
            .text_color(rgb(palette.on_chosen)),
        ButtonKind::Secondary => button.text_color(rgb(palette.text_primary)),
        ButtonKind::Destructive => button.text_color(rgb(palette.error)),
    };
    let button = if enabled {
        button.tab_index(0).owns_clicks().on_click(on_click)
    } else {
        button.opacity(
            crate::platform::theme_config::theme()
                .opacity
                .disabled_control,
        )
    };
    let button = pressable(button).enabled(enabled);
    match kind {
        ButtonKind::Primary => button.hover_style(|style| {
            style
                .bg(rgb(palette.chosen_hover))
                .border_color(rgb(palette.chosen_hover))
        }),
        ButtonKind::Secondary => button.hover_style(|style| style.bg(rgba(palette.surface_hover))),
        ButtonKind::Destructive => {
            button.hover_style(|style| style.bg(rgba(palette.error_hover_surface)))
        }
    }
}

/// A round button `size` pixels across showing only an icon, labelled for
/// accessibility.
pub(super) fn icon_button(
    id: &'static str,
    label: &'static str,
    enabled: bool,
    size: f32,
    icon: impl IntoElement,
    palette: ThemeColors,
    on_click: ClickHandler,
) -> PressScaleElement<Stateful<Div>> {
    let button = h_stack()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .aria_disabled(!enabled)
        .focus_ring(palette)
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(size))
        .rounded(px(size / 2.0))
        .opacity(if enabled {
            1.0
        } else {
            crate::platform::theme_config::theme()
                .opacity
                .disabled_control
        })
        .child(icon);
    let button = if enabled {
        button
            .tab_index(0)
            .hover_style(|style| style.bg(rgba(palette.hover_surface)))
            .owns_clicks()
            .on_click(on_click)
    } else {
        button
    };
    icon_pressable(button).enabled(enabled)
}

/// A square toolbar button showing an icon, dimmed and inert while disabled.
pub(super) fn toolbar_button(
    id: &'static str,
    label: &'static str,
    tab_index: isize,
    enabled: bool,
    icon: impl IntoElement,
    palette: ThemeColors,
    on_click: ClickHandler,
) -> PressScaleElement<Stateful<Div>> {
    let button = h_stack()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .aria_disabled(!enabled)
        .focus_ring(palette)
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(metrics::TOOLBAR_BUTTON_SIZE))
        .rounded(px(metrics::control_radius()))
        .child(icon);
    let button = if enabled {
        button
            .tab_index(tab_index)
            .hover_style(|style| style.bg(rgba(palette.hover_surface)))
            .owns_clicks()
            .on_click(on_click)
    } else {
        button.opacity(
            crate::platform::theme_config::theme()
                .opacity
                .disabled_control,
        )
    };
    icon_pressable(button).enabled(enabled)
}

/// The button that opens the browser menu.
pub(super) fn menu_button(
    id: &'static str,
    tab_index: isize,
    palette: ThemeColors,
    on_click: ClickHandler,
) -> PressScaleElement<Stateful<Div>> {
    let button = h_stack()
        .id(id)
        .role(Role::Button)
        .aria_label("Open browser menu")
        .tab_index(tab_index)
        .focus_ring(palette)
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(metrics::TOOLBAR_BUTTON_SIZE))
        .rounded(px(metrics::control_radius()))
        .hover_style(|style| style.bg(rgba(palette.hover_surface)))
        .owns_clicks();
    icon_pressable(button).on_click(on_click).child(more_icon(
        palette.text_secondary,
        metrics::TOOLBAR_ICON_SIZE,
    ))
}
