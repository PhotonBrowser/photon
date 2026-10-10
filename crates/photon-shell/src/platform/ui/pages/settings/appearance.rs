//! The appearance section: light, dark or following macOS, the window's
//! colour, how much of the desktop shows through the browser, and where the
//! tabs go.

use gpui::{App, prelude::*, px};
use photon_core::{TabLayout, ThemeMode, Transparency, WorkspaceLayout as SavedWorkspaceLayout};
use std::rc::Rc;

use super::super::super::color_picker::color_picker;
use super::super::super::controls::{Choice, choices};
use super::super::super::dropdown::dropdown;
use super::super::super::layout::v_stack;
use super::super::super::metrics;
use super::super::super::settings::Settings;
use super::super::super::theme::ThemeColors;
use super::super::super::workspace_layout::WorkspaceLayout;
use super::super::change;
use super::super::layout::{label, secondary_text};

pub(super) fn settings(palette: ThemeColors, cx: &App) -> impl IntoElement {
    let current = Settings::get(cx);
    let themes = [
        (ThemeMode::System, "System"),
        (ThemeMode::Light, "Light"),
        (ThemeMode::Dark, "Dark"),
    ]
    .into_iter()
    .map(|(theme, label)| Choice {
        label: label.into(),
        chosen: current.theme == theme,
        on_choose: change(move |settings| settings.theme = theme),
    })
    .collect();
    let transparencies = [
        (Transparency::Off, "Off"),
        (Transparency::Subtle, "Subtle"),
        (Transparency::Clear, "Clear"),
    ]
    .into_iter()
    .map(|(transparency, label)| Choice {
        label: label.into(),
        chosen: current.transparency == transparency,
        on_choose: change(move |settings| settings.transparency = transparency),
    })
    .collect();
    let space = current.spaces.active();
    let colors = color_picker(
        "settings-window-color",
        space.color,
        space.gradient,
        palette,
        Rc::new(|color, gradient, _, cx| {
            Settings::update(cx, |settings| {
                let id = settings.spaces.active().id;
                if let Some(space) = settings.spaces.get_mut(id) {
                    space.color = color;
                    space.gradient = gradient;
                }
            });
        }),
    );
    let tab_layouts = [
        (TabLayout::Horizontal, "Horizontal"),
        (TabLayout::Vertical, "Vertical"),
    ]
    .into_iter()
    .map(|(layout, label)| Choice {
        label: label.into(),
        chosen: current.workspace.default_preset_id
            == match layout {
                TabLayout::Horizontal => photon_core::HORIZONTAL_PRESET_ID,
                TabLayout::Vertical => photon_core::VERTICAL_PRESET_ID,
            },
        on_choose: change(move |settings| {
            settings.tab_layout = layout;
            settings.workspace.set_builtin_layout(layout);
        }),
    })
    .collect();
    let active_space_id = current.spaces.active().id;
    let active_layout_override = current
        .spaces
        .active()
        .layout_override
        .as_ref()
        .and_then(WorkspaceLayout::from_saved)
        .map(|layout| layout.saved().clone());
    let follow_default = current.spaces.active().layout_override.is_none();
    let horizontal_layout = SavedWorkspaceLayout::horizontal();
    let vertical_layout = SavedWorkspaceLayout::vertical();
    let follows_horizontal = active_layout_override.as_ref() == Some(&horizontal_layout);
    let follows_vertical = active_layout_override.as_ref() == Some(&vertical_layout);
    let mut space_layouts = vec![
        Choice {
            label: "Use profile default".into(),
            chosen: follow_default,
            on_choose: change(move |settings| {
                settings.spaces.clear_layout_override(active_space_id);
            }),
        },
        Choice {
            label: "Horizontal".into(),
            chosen: follows_horizontal,
            on_choose: change(move |settings| {
                settings
                    .spaces
                    .set_layout_override(active_space_id, &SavedWorkspaceLayout::horizontal());
            }),
        },
        Choice {
            label: "Vertical".into(),
            chosen: follows_vertical,
            on_choose: change(move |settings| {
                settings
                    .spaces
                    .set_layout_override(active_space_id, &SavedWorkspaceLayout::vertical());
            }),
        },
    ];
    if !follow_default && !follows_horizontal && !follows_vertical {
        space_layouts.push(Choice {
            label: "Custom layout".into(),
            chosen: true,
            on_choose: change(move |_| {}),
        });
    }
    v_stack()
        .gap(px(metrics::INTERNAL_PAGE_SECTION_GAP))
        .child(
            v_stack()
                .gap(px(metrics::MENU_ITEM_GAP))
                .child(label("Theme", palette))
                .child(choices("settings-theme", "Theme", themes, palette)),
        )
        .child(
            v_stack()
                .gap(px(metrics::MENU_ITEM_GAP))
                .child(label("Window colour", palette))
                .child(colors)
                .child(secondary_text(
                    "The colour of the space shown. Each space has its own.",
                    palette,
                )),
        )
        .child(
            v_stack()
                .gap(px(metrics::MENU_ITEM_GAP))
                .child(label("Transparency", palette))
                .child(choices(
                    "settings-transparency",
                    "Transparency",
                    transparencies,
                    palette,
                ))
                .child(secondary_text(
                    "How much of the desktop shows through tabs, toolbars, menus and pages.",
                    palette,
                )),
        )
        .child(
            v_stack()
                .gap(px(metrics::MENU_ITEM_GAP))
                .child(label("Default layout", palette))
                .child(dropdown(
                    "settings-default-layout",
                    "Default layout",
                    tab_layouts,
                    palette,
                ))
                .child(secondary_text(
                    "Used by new spaces and spaces set to use the profile default.",
                    palette,
                ))
                .child(label("Layout for this space", palette))
                .child(dropdown(
                    "settings-space-layout",
                    "Layout for this space",
                    space_layouts,
                    palette,
                ))
                .child(secondary_text(
                    "Choosing a layout saves a copy for this space. Use the profile default to follow future changes.",
                    palette,
                )),
        )
}
