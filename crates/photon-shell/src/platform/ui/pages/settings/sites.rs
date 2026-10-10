//! The sites section: whether sites may open windows of their own.

use gpui::{App, prelude::*, px};
use photon_core::PopupPolicy;

use super::super::super::controls::{Choice, choices};
use super::super::super::layout::v_stack;
use super::super::super::settings::Settings;
use super::super::super::{metrics, theme::ThemeColors};
use super::super::change;
use super::super::layout::{label, secondary_text};

pub(super) fn settings(palette: ThemeColors, cx: &App) -> impl IntoElement {
    let current = Settings::get(cx).popup_policy;
    let options = [
        (PopupPolicy::Ask, "Ask"),
        (PopupPolicy::Allow, "Allow"),
        (PopupPolicy::Block, "Block"),
    ]
    .into_iter()
    .map(|(policy, label)| Choice {
        label: label.into(),
        chosen: current == policy,
        on_choose: change(move |settings| settings.popup_policy = policy),
    })
    .collect();
    v_stack()
        .gap(px(metrics::MENU_ITEM_GAP))
        .child(label("Pop-up windows", palette))
        .child(choices(
            "settings-popups",
            "Pop-up windows",
            options,
            palette,
        ))
        .child(secondary_text(
            "When a site asks to open a new window.",
            palette,
        ))
}
