//! The features section: optional browser features and their availability.

use gpui::{App, prelude::*, px};
use photon_core::{FEATURE_REGISTRY, Feature};

use super::super::super::layout::v_stack;
use super::super::super::settings::Settings;
use super::super::super::{metrics, theme::ThemeColors};
use super::super::change;
use super::super::controls::switch_row;
use super::super::layout::{group, secondary_text};

pub(super) fn settings(palette: ThemeColors, cx: &App) -> impl IntoElement {
    let current = Settings::get(cx).features.clone();
    let rows = FEATURE_REGISTRY.builtins().iter().map(|definition| {
        let feature = definition.feature;
        let enabled = current.is_configured_enabled(feature);
        v_stack()
            .gap(px(metrics::MENU_ITEM_GAP))
            .child(group(palette).child(switch_row(
                definition.setting_id,
                definition.name,
                enabled,
                palette,
                change(move |settings| toggle(&mut settings.features, feature)),
            )))
            .child(secondary_text(definition.description, palette))
    });

    v_stack()
        .gap(px(metrics::INTERNAL_PAGE_SECTION_GAP))
        .children(rows)
}

fn toggle(settings: &mut photon_core::FeatureSettings, feature: Feature) {
    if settings.is_configured_enabled(feature) {
        settings.disable(feature);
    } else {
        settings.enable(feature);
    }
}
