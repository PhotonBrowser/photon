//! The sidebar's footer: settings at the left, the spaces in the middle, and
//! the browser menu at the right. Collapsed to icons, the spaces stack
//! above the browser menu.

use gpui::{MouseButton, Role, div, prelude::*, px, rgb, rgba};

use super::super::button::{menu_button, toolbar_button};
use super::super::controls::FocusRing;
use super::super::controls::{OwnsClicks, Pressable};
use super::super::icons::{add_icon, settings_icon};
use super::super::layout::{h_stack, v_stack};
use super::super::tooltip::label_tooltip;
use super::super::{ClickHandler, MouseDownHandler, metrics, theme::ThemeColors};

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
    pub(in super::super) on_context_menu: MouseDownHandler,
}

pub(in super::super) fn footer(
    spaces: Vec<SpaceDot>,
    actions: FooterActions,
    collapsed: bool,
    palette: ThemeColors,
) -> gpui::AnyElement {
    let menu = menu_button("sidebar-menu", 2, palette, actions.menu)
        .tooltip(label_tooltip("Open browser menu", palette));
    if collapsed {
        let spacing = metrics::SidebarSpacing::new(collapsed);
        let menu = super::collapsed_tile(menu);
        return v_stack()
            .w_full()
            .flex_shrink_0()
            .items_center()
            .py(px(spacing.padding))
            .gap(px(spacing.item_gap))
            .child(space_switcher(v_stack(), spaces, palette))
            .child(menu)
            .into_any_element();
    }
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
            space_switcher(h_stack(), spaces, palette).child(toolbar_button(
                "sidebar-new-space",
                "New space",
                1,
                true,
                add_icon(palette.text_secondary, metrics::TOOLBAR_ICON_SIZE),
                palette,
                actions.new_space,
            )),
        )
        .child(menu)
        .into_any_element()
}

/// The spaces as dots in `stack`, a row or a column. One space needs no
/// switcher, so it shows none.
fn space_switcher(
    stack: gpui::Div,
    spaces: Vec<SpaceDot>,
    palette: ThemeColors,
) -> gpui::Stateful<gpui::Div> {
    let dots = (spaces.len() > 1).then(|| {
        spaces
            .into_iter()
            .enumerate()
            .map(move |(index, space)| space_dot(index, space, palette))
    });
    stack
        .id("sidebar-spaces")
        .role(Role::TabList)
        .aria_label("Spaces")
        .items_center()
        .gap(px(metrics::SPACE_DOT_GAP))
        .children(dots.into_iter().flatten())
}

/// A dot of the space's colour, larger and ringed while it is shown.
fn space_dot(index: usize, space: SpaceDot, palette: ThemeColors) -> impl IntoElement {
    let tooltip = label_tooltip(space.name.clone(), palette);
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
        .tab_index(0)
        .focus_ring(palette)
        .size(px(metrics::SPACE_DOT_BUTTON_SIZE))
        .items_center()
        .justify_center()
        .rounded_full()
        .hover(|style| style.bg(rgba(palette.hover_surface)))
        .owns_clicks()
        .pressable()
        .on_mouse_down(MouseButton::Right, space.on_context_menu)
        .on_click(space.on_select)
        .tooltip(tooltip)
        .child(
            div()
                .size(px(size))
                .rounded_full()
                .bg(rgb(space.color))
                .when(!space.active, |dot| {
                    dot.opacity(
                        crate::platform::theme_config::theme()
                            .opacity
                            .space_dot_idle,
                    )
                }),
        )
}
