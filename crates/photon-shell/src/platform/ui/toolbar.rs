//! The address toolbar under the horizontal tab strip: navigation, the
//! address field and the browser menu.

use gpui::{prelude::*, px};

use super::button::{menu_button, toolbar_button};
use super::icons::{back_icon, close_icon, forward_icon, reload_icon};
use super::layout::h_stack;
use super::sidebar::{NavigationActions, NavigationState};
use super::{ClickHandler, metrics, theme::ThemeColors};

pub(super) fn address_toolbar(
    omnibox: impl IntoElement,
    state: NavigationState,
    actions: NavigationActions,
    menu_open: bool,
    on_menu: ClickHandler,
    palette: ThemeColors,
) -> impl IntoElement {
    let (color, size) = (palette.text_secondary, metrics::TOOLBAR_ICON_SIZE);
    let (reload_label, reload) = if state.loading {
        ("Stop loading", close_icon(color, size).into_any_element())
    } else {
        ("Reload page", reload_icon(color, size).into_any_element())
    };
    h_stack()
        .w_full()
        .h(px(metrics::TOOLBAR_HEIGHT))
        .items_center()
        .gap(px(metrics::TOOLBAR_CONTROL_GAP))
        .px(px(metrics::TOOLBAR_HORIZONTAL_INSET))
        .tab_group()
        .child(toolbar_button(
            "toolbar-back",
            "Go back",
            0,
            state.can_go_back,
            back_icon(color, size),
            palette,
            actions.back,
        ))
        .child(toolbar_button(
            "toolbar-forward",
            "Go forward",
            1,
            state.can_go_forward,
            forward_icon(color, size),
            palette,
            actions.forward,
        ))
        .child(toolbar_button(
            "toolbar-reload",
            reload_label,
            2,
            state.can_reload || state.loading,
            reload,
            palette,
            actions.reload,
        ))
        .child(omnibox)
        .child(menu_button("toolbar-menu", 4, menu_open, palette, on_menu))
}
