//! The sidebar's header for the current space and its footer.

use gpui::{MouseButton, Role, div, prelude::*, px, rgb, rgba};
use photon_core::Space;

use super::super::icons::{more_icon, settings_icon};
use super::super::layout::h_stack;
use super::super::{ClickHandler, metrics, theme::ThemeColors};
use super::navigation::button;

/// The current space's icon and name, over its tabs.
pub(in super::super) fn space_header(space: &Space, palette: ThemeColors) -> impl IntoElement {
    h_stack()
        .id("sidebar-space")
        .role(Role::Heading)
        .aria_level(2)
        .aria_label(space.name.clone())
        .items_center()
        .gap(px(metrics::OMNIBOX_GAP))
        .h(px(metrics::SIDEBAR_TAB_HEIGHT))
        .px(px(metrics::SIDEBAR_TAB_PADDING))
        .text_size(px(metrics::SIDEBAR_FONT_SIZE))
        .text_color(rgb(palette.text_secondary))
        .child(space.icon.clone())
        .child(space.name.clone())
}

pub(in super::super) struct FooterActions {
    pub(in super::super) settings: ClickHandler,
    pub(in super::super) menu: ClickHandler,
}

/// Settings at the left, the current space in the middle, and the browser
/// menu at the right.
pub(in super::super) fn footer(
    space: &Space,
    menu_open: bool,
    actions: FooterActions,
    palette: ThemeColors,
) -> impl IntoElement {
    let menu = h_stack()
        .id("sidebar-menu")
        .role(Role::Button)
        .aria_label(if menu_open {
            "Close browser menu"
        } else {
            "Open browser menu"
        })
        .aria_expanded(menu_open)
        .tab_index(0)
        .focus_visible(|style| style.border_1().border_color(rgb(palette.chosen)))
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(metrics::TOOLBAR_BUTTON_SIZE))
        .rounded(px(metrics::CONTROL_RADIUS))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(actions.menu)
        .child(more_icon(
            palette.text_secondary,
            metrics::TOOLBAR_ICON_SIZE,
        ));
    let menu = if menu_open {
        menu.bg(rgba(palette.selected_surface))
    } else {
        menu.hover(|style| style.bg(rgba(palette.hover_surface)))
    };
    h_stack()
        .w_full()
        .h(px(metrics::SIDEBAR_FOOTER_HEIGHT))
        .flex_shrink_0()
        .items_center()
        .justify_between()
        .px(px(metrics::SIDEBAR_PADDING))
        .child(button(
            "sidebar-settings",
            "Settings",
            0,
            true,
            settings_icon(palette.text_secondary, metrics::TOOLBAR_ICON_SIZE),
            palette,
            actions.settings,
        ))
        .child(
            div()
                .id("sidebar-current-space")
                .role(Role::Status)
                .aria_label(format!("Space: {}", space.name))
                .text_size(px(metrics::SIDEBAR_FONT_SIZE))
                .child(space.icon.clone()),
        )
        .child(menu)
}
