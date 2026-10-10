//! The search section: which engine searches from the address bar go to,
//! and how much of a page's address the bar shows.

use gpui::{App, prelude::*, px};

use super::super::super::controls::{Choice, choices};
use super::super::super::layout::v_stack;
use super::super::super::settings::Settings;
use super::super::super::{metrics, theme::ThemeColors};
use super::super::change;
use super::super::controls::switch_row;
use super::super::layout::{group, secondary_text};

pub(super) fn settings(palette: ThemeColors, cx: &App) -> impl IntoElement {
    let current = Settings::get(cx);
    let engine = current.default_search_engine.clone();
    let full_address = current.show_full_address;
    let options = Settings::search_engines(cx)
        .engines()
        .iter()
        .map(|candidate| {
            let id = candidate.id.to_string();
            Choice {
                label: candidate.name.to_string().into(),
                chosen: engine == id,
                on_choose: change(move |settings| settings.default_search_engine.clone_from(&id)),
            }
        })
        .collect();
    v_stack()
        .gap(px(metrics::INTERNAL_PAGE_SECTION_GAP))
        .child(choices("settings-search", "Search engine", options, palette))
        .child(
            v_stack()
                .gap(px(metrics::MENU_ITEM_GAP))
                .child(
                    group(palette).child(switch_row(
                        "settings-show-full-address",
                        "Show full address",
                        full_address,
                        palette,
                        change(move |settings| settings.show_full_address = !full_address),
                    )),
                )
                .child(secondary_text(
                    "When off, the address bar shows only the site, such as github.com, until you click it.",
                    palette,
                )),
        )
}
