//! The new tab page section: the same options as the page's customise menu,
//! laid out as settings.

use gpui::{App, prelude::*, px};

use super::super::super::button::{ButtonSize, button};
use super::super::super::controls::choices;
use super::super::super::layout::{h_stack, v_stack};
use super::super::super::{metrics, theme::ThemeColors};
use super::super::controls::switch_row;
use super::super::layout::{group, label};
use super::super::new_tab::LayoutOptions;

pub(super) fn settings(palette: ThemeColors, cx: &App) -> impl IntoElement {
    let options = LayoutOptions::read(cx);
    let mut rows = group(palette)
        .child(switch_row(
            "settings-new-tab-logo",
            "Show logo",
            options.show_logo,
            palette,
            options.toggle_logo(),
        ))
        .child(switch_row(
            "settings-new-tab-shortcuts",
            "Show shortcuts",
            options.show_shortcuts,
            palette,
            options.toggle_shortcuts(),
        ));
    if options.has_removed {
        rows = rows.child(
            h_stack()
                .px(px(metrics::MENU_ITEM_HORIZONTAL_PADDING))
                .py(px(metrics::MENU_ITEM_GAP))
                .child(button(
                    "settings-new-tab-restore",
                    "Restore removed shortcuts",
                    false,
                    ButtonSize::Small,
                    palette,
                    options.restore_removed(),
                )),
        );
    }
    let mut section = v_stack()
        .gap(px(metrics::INTERNAL_PAGE_GROUP_GAP))
        .child(rows);
    if options.show_shortcuts {
        section = section.child(
            v_stack()
                .gap(px(metrics::MENU_ITEM_GAP))
                .child(label("Number of shortcuts", palette))
                .child(h_stack().child(choices(
                    "settings-new-tab-count",
                    "Number of shortcuts",
                    LayoutOptions::shortcut_counts(cx),
                    palette,
                ))),
        );
    }
    section
}
