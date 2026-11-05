//! Shortcut count and restore controls for the new tab page.

use gpui::{App, prelude::*, px};

use super::super::super::button::{ButtonKind, ButtonSize, button};
use super::super::super::controls::choices;
use super::super::super::layout::{h_stack, v_stack};
use super::super::super::{metrics, theme::ThemeColors};
use super::super::layout::{group, label};
use super::super::new_tab::LayoutOptions;

pub(super) fn settings(palette: ThemeColors, cx: &App) -> impl IntoElement {
    let options = LayoutOptions::read(cx);
    let mut section = v_stack().gap(px(metrics::INTERNAL_PAGE_GROUP_GAP));
    if options.has_removed {
        section = section.child(
            group(palette).child(
                h_stack()
                    .px(px(metrics::MENU_ITEM_HORIZONTAL_PADDING))
                    .py(px(metrics::MENU_ITEM_GAP))
                    .child(button(
                        "settings-new-tab-restore",
                        "Restore removed shortcuts",
                        ButtonKind::Secondary,
                        true,
                        ButtonSize::Small,
                        palette,
                        options.restore_removed(),
                    )),
            ),
        );
    }
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
