//! Text buttons for alerts and dialogs.

use gpui::{Div, Role, Stateful, prelude::*, px, rgb, rgba};

use super::ClickHandler;
use super::controls::OwnsClicks;
use super::icons::more_icon;
use super::layout::{ControlSurface, h_stack};
use super::{metrics, theme::ThemeColors};

/// A button's size.
#[derive(Clone, Copy)]
pub(super) enum ButtonSize {
    /// For dialogs and alerts.
    Regular,
    /// For compact surfaces such as status chips.
    Small,
}

/// A labelled button. The `primary` button is the default action and is
/// filled; others are plain until hovered.
pub(super) fn button(
    id: &'static str,
    label: &'static str,
    primary: bool,
    size: ButtonSize,
    palette: ThemeColors,
    on_click: ClickHandler,
) -> impl IntoElement {
    let (height, min_width, padding, font_size) = match size {
        ButtonSize::Regular => (
            metrics::BUTTON_HEIGHT,
            metrics::BUTTON_MIN_WIDTH,
            metrics::BUTTON_HORIZONTAL_PADDING,
            metrics::BUTTON_FONT_SIZE,
        ),
        ButtonSize::Small => (
            metrics::SMALL_BUTTON_HEIGHT,
            0.0,
            metrics::SMALL_BUTTON_HORIZONTAL_PADDING,
            metrics::SMALL_BUTTON_FONT_SIZE,
        ),
    };
    let button = h_stack()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .tab_index(0)
        .focus_visible(|style| {
            style.border_1().border_color(rgb(if primary {
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
        .text_size(px(font_size))
        .owns_clicks()
        .on_click(on_click)
        .child(label);

    if primary {
        button
            .bg(rgb(palette.chosen))
            .border_color(rgb(palette.chosen))
            .text_color(rgb(palette.on_chosen))
            .hover(|style| {
                style
                    .bg(rgb(palette.chosen_hover))
                    .border_color(rgb(palette.chosen_hover))
            })
    } else {
        button
            .text_color(rgb(palette.text_primary))
            .hover(|style| style.bg(rgba(palette.selected_surface)))
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
) -> impl IntoElement {
    let button = h_stack()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .aria_disabled(!enabled)
        .focus_visible(|style| style.border_1().border_color(rgb(palette.chosen)))
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(size))
        .rounded(px(size / 2.0))
        .opacity(if enabled {
            1.0
        } else {
            metrics::DISABLED_OPACITY
        })
        .child(icon);
    if !enabled {
        return button;
    }
    button
        .tab_index(0)
        .hover(|style| style.bg(rgba(palette.hover_surface)))
        .owns_clicks()
        .on_click(on_click)
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
) -> Stateful<Div> {
    let button = h_stack()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .aria_disabled(!enabled)
        .focus_visible(|style| style.border_1().border_color(rgb(palette.chosen)))
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(metrics::TOOLBAR_BUTTON_SIZE))
        .rounded(px(metrics::CONTROL_RADIUS))
        .child(icon);
    if !enabled {
        return button.opacity(metrics::DISABLED_OPACITY);
    }
    button
        .tab_index(tab_index)
        .hover(|style| style.bg(rgba(palette.hover_surface)))
        .owns_clicks()
        .on_click(on_click)
}

/// The button that opens the browser menu.
pub(super) fn menu_button(
    id: &'static str,
    tab_index: isize,
    palette: ThemeColors,
    on_click: ClickHandler,
) -> Stateful<Div> {
    h_stack()
        .id(id)
        .role(Role::Button)
        .aria_label("Open browser menu")
        .tab_index(tab_index)
        .focus_visible(|style| style.border_1().border_color(rgb(palette.chosen)))
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(metrics::TOOLBAR_BUTTON_SIZE))
        .rounded(px(metrics::CONTROL_RADIUS))
        .hover(|style| style.bg(rgba(palette.hover_surface)))
        .owns_clicks()
        .on_click(on_click)
        .child(more_icon(
            palette.text_secondary,
            metrics::TOOLBAR_ICON_SIZE,
        ))
}
