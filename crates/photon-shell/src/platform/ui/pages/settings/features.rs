//! The features section: optional browser features and their availability.

use gpui::{App, prelude::*, px};
use photon_core::{FEATURE_REGISTRY, Feature, FeatureCategory};

use super::super::super::layout::v_stack;
use super::super::super::settings::Settings;
use super::super::super::{metrics, theme::ThemeColors};
use super::super::change;
use super::super::controls::switch_row;
use super::super::layout::{group, label, secondary_text};

pub(super) fn settings(palette: ThemeColors, cx: &App) -> impl IntoElement {
    let current = Settings::get(cx).clone();
    let sections = FeatureCategory::ALL.into_iter().map(|category| {
        let rows = FEATURE_REGISTRY
            .builtins()
            .iter()
            .filter(|definition| definition.category == category)
            .map(|definition| {
                let feature = definition.feature;
                let enabled = current.is_feature_enabled(feature);
                v_stack()
                    .gap(px(metrics::MENU_ITEM_GAP))
                    .child(group(palette).child(switch_row(
                        definition.setting_id,
                        definition.name,
                        enabled,
                        palette,
                        change(move |settings| toggle(settings, feature)),
                    )))
                    .child(secondary_text(definition.description, palette))
            });

        v_stack()
            .gap(px(metrics::INTERNAL_PAGE_GROUP_GAP))
            .child(label(category.name(), palette))
            .children(rows)
    });

    v_stack()
        .gap(px(metrics::INTERNAL_PAGE_SECTION_GAP))
        .children(sections)
}

fn toggle(settings: &mut photon_core::BrowserSettings, feature: Feature) {
    let enabled = settings.is_feature_enabled(feature);
    settings.set_feature_enabled(feature, !enabled);
}
