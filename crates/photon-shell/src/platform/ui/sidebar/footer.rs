//! The sidebar's footer: settings at the left, the spaces in the middle, and
//! the browser menu at the right.

use gpui::{App, MouseButton, MouseDownEvent, Role, Window, div, prelude::*, px, rgb, rgba};

use super::super::button::{menu_button, toolbar_button};
use super::super::controls::toggled;
use super::super::icons::{add_icon, settings_icon};
use super::super::layout::h_stack;
use super::super::{ClickHandler, metrics, theme::ThemeColors};

pub(in super::super) struct FooterActions {
    pub(in super::super) settings: ClickHandler,
    pub(in super::super) menu: ClickHandler,
    pub(in super::super) new_space: ClickHandler,
}

/// A space, shown as a dot of its colour.
pub(in super::super) struct SpaceDot {
    pub(in super::super) name: String,
    /// The space's colour, as an RGB token.
    pub(in super::super) color: u32,
    pub(in super::super) active: bool,
    pub(in super::super) on_select: ClickHandler,
    /// Right-click opens the space's menu.
    pub(in super::super) on_context_menu: Box<dyn Fn(&MouseDownEvent, &mut Window, &mut App)>,
}

pub(in super::super) fn footer(
    menu_open: bool,
    spaces: Vec<SpaceDot>,
    actions: FooterActions,
    palette: ThemeColors,
) -> impl IntoElement {
    // One space needs no switcher; the button to add another stays.
    let dots = (spaces.len() > 1).then(|| {
        spaces
            .into_iter()
            .enumerate()
            .map(move |(index, space)| space_dot(index, space, palette))
    });
    h_stack()
        .w_full()
        .h(px(metrics::SIDEBAR_FOOTER_HEIGHT))
        .flex_shrink_0()
        .items_center()
        .justify_between()
        .px(px(metrics::SIDEBAR_PADDING))
        .child(toolbar_button(
            "sidebar-settings",
            "Settings",
            0,
            true,
            settings_icon(palette.text_secondary, metrics::TOOLBAR_ICON_SIZE),
            palette,
            actions.settings,
        ))
        .child(
            h_stack()
                .id("sidebar-spaces")
                .role(Role::TabList)
                .aria_label("Spaces")
                .items_center()
                .gap(px(metrics::SPACE_DOT_GAP))
                .children(dots.into_iter().flatten())
                .child(toolbar_button(
                    "sidebar-new-space",
                    "New space",
                    1,
                    true,
                    add_icon(palette.text_secondary, metrics::TOOLBAR_ICON_SIZE),
                    palette,
                    actions.new_space,
                )),
        )
        .child(menu_button(
            "sidebar-menu",
            2,
            menu_open,
            palette,
            actions.menu,
        ))
}

/// A dot of the space's colour, larger and ringed while it is shown.
fn space_dot(index: usize, space: SpaceDot, palette: ThemeColors) -> impl IntoElement {
    let size = if space.active {
        metrics::SPACE_DOT_ACTIVE_SIZE
    } else {
        metrics::SPACE_DOT_SIZE
    };
    h_stack()
        .id(("sidebar-space", index))
        .role(Role::Tab)
        .aria_label(space.name)
        .aria_selected(space.active)
        .aria_toggled(toggled(space.active))
        .tab_index(0)
        .focus_visible(|style| style.border_1().border_color(rgb(palette.chosen)))
        .size(px(metrics::SPACE_DOT_BUTTON_SIZE))
        .items_center()
        .justify_center()
        .rounded_full()
        .hover(|style| style.bg(rgba(palette.hover_surface)))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Right, space.on_context_menu)
        .on_click(space.on_select)
        .child(
            div()
                .size(px(size))
                .rounded_full()
                .bg(rgb(space.color))
                .when(!space.active, |dot| {
                    dot.opacity(metrics::SPACE_DOT_IDLE_OPACITY)
                }),
        )
}
