//! Shared controls for choosing the tab arrangement in a workspace preset.

use gpui::{prelude::*, px};
use photon_core::{SidebarSide, TabLayout};

use super::controls::{Choice, choices};
use super::layout::{form_field, v_stack};
use super::{ClickHandler, metrics, theme::ThemeColors};

const TAB_LAYOUTS: [(TabLayout, &str); 2] = [
    (TabLayout::Horizontal, "Horizontal"),
    (TabLayout::Vertical, "Vertical"),
];

const SIDEBAR_SIDES: [(SidebarSide, &str); 2] =
    [(SidebarSide::Left, "Left"), (SidebarSide::Right, "Right")];

/// The shared tab and sidebar choices used by preset creation and editing.
pub(super) fn fields(
    tab_layout_id: &'static str,
    sidebar_side_id: &'static str,
    tab_layout: TabLayout,
    sidebar_side: SidebarSide,
    palette: ThemeColors,
    on_tab_layout: impl Fn(TabLayout) -> ClickHandler,
    on_sidebar_side: impl Fn(SidebarSide) -> ClickHandler,
) -> impl IntoElement {
    let tab_choices = TAB_LAYOUTS
        .into_iter()
        .map(|(layout, name)| Choice {
            label: name.into(),
            chosen: tab_layout == layout,
            on_choose: on_tab_layout(layout),
        })
        .collect();
    let sidebar_choices = SIDEBAR_SIDES
        .into_iter()
        .map(|(side, name)| Choice {
            label: name.into(),
            chosen: sidebar_side == side,
            on_choose: on_sidebar_side(side),
        })
        .collect();

    v_stack()
        .gap(px(metrics::INTERNAL_PAGE_GROUP_GAP))
        .child(form_field(
            "Tabs",
            choices(tab_layout_id, "Tab layout", tab_choices, palette),
            palette,
        ))
        .child(form_field(
            "Sidebar side",
            choices(sidebar_side_id, "Sidebar side", sidebar_choices, palette),
            palette,
        ))
}
