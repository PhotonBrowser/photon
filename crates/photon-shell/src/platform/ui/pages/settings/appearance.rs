//! Appearance settings for the profile and the active space.

use gpui::{App, ElementId, FontWeight, Window, div, prelude::*, px, rgba};
use photon_core::{Transparency, WindowBackgroundStyle};
use std::rc::Rc;

use super::super::super::button::{ButtonKind, ButtonSize, button};
use super::super::super::controls::{Choice, choices};
use super::super::super::dropdown::dropdown;
use super::super::super::layout::{form_field, h_stack, v_stack};
use super::super::super::metrics;
use super::super::super::settings::Settings;
use super::super::super::theme::ThemeColors;
use super::super::super::workspace_layout::WorkspaceLayout;
use super::super::ClickHandler;
use super::super::change;
use super::super::layout::{label, secondary_text};

pub(super) fn settings(
    _window: &mut Window,
    palette: ThemeColors,
    cx: &mut App,
    create_preset: ClickHandler,
    edit_preset: Rc<dyn Fn(String) -> ClickHandler>,
    delete_preset: Rc<dyn Fn(String) -> ClickHandler>,
) -> impl IntoElement {
    let current = Settings::get(cx).clone();
    let transparencies = [
        (Transparency::Off, "Off"),
        (Transparency::Subtle, "Subtle"),
        (Transparency::Clear, "Clear"),
    ]
    .into_iter()
    .map(|(transparency, label)| Choice {
        label: label.into(),
        chosen: current.transparency_percent.is_none() && current.transparency == transparency,
        on_choose: change(move |settings| {
            settings.transparency = transparency;
            settings.transparency_percent = None;
        }),
    })
    .collect();
    let window_background_styles = [
        (WindowBackgroundStyle::LiquidGlass, "Liquid Glass"),
        (WindowBackgroundStyle::Blurred, "Blurred"),
    ]
    .into_iter()
    .map(|(style, label)| Choice {
        label: label.into(),
        chosen: current.window_background_style == style,
        on_choose: change(move |settings| settings.window_background_style = style),
    })
    .collect();

    let active_space = current.spaces.active();
    let active_space_id = active_space.id;
    let active_layout_override = active_space.layout_override.clone();
    let active_preset_override_id = active_space.preset_override_id.clone();

    let default_preset_choices = current
        .workspace
        .presets
        .iter()
        .map(|preset| {
            let id = preset.id.clone();
            let layout = preset.layout.clone();
            Choice {
                label: preset.name.clone().into(),
                chosen: current.workspace.default_preset_id == preset.id,
                on_choose: change(move |settings| {
                    settings.workspace.set_default_preset(&id);
                    settings.tab_layout = layout.tab_layout();
                }),
            }
        })
        .collect();

    let active_layout_override = active_layout_override
        .as_ref()
        .and_then(WorkspaceLayout::from_saved)
        .map(|layout| layout.saved().clone());
    let follows_default = active_layout_override.is_none() && active_preset_override_id.is_none();
    let selected_space_preset = active_preset_override_id
        .as_ref()
        .and_then(|selected_id| {
            current
                .workspace
                .presets
                .iter()
                .position(|preset| &preset.id == selected_id)
        })
        .or_else(|| {
            active_layout_override.as_ref().and_then(|layout| {
                current
                    .workspace
                    .presets
                    .iter()
                    .position(|preset| &preset.layout == layout)
            })
        });
    let mut space_layout_choices = vec![Choice {
        label: "Use profile default".into(),
        chosen: follows_default,
        on_choose: change(move |settings| {
            settings.spaces.clear_layout_override(active_space_id);
        }),
    }];
    space_layout_choices.extend(current.workspace.presets.iter().enumerate().map(
        |(index, preset)| {
            let id = preset.id.clone();
            Choice {
                label: preset.name.clone().into(),
                chosen: selected_space_preset == Some(index),
                on_choose: change(move |settings| {
                    settings.spaces.set_preset_override(active_space_id, &id);
                }),
            }
        },
    ));
    if !follows_default && selected_space_preset.is_none() {
        space_layout_choices.push(Choice {
            label: "Current layout".into(),
            chosen: true,
            on_choose: change(|_| {}),
        });
    }

    let presets = current.workspace.presets.iter().map(|preset| {
        let id = preset.id.clone();
        let preset_name = preset.name.clone();
        let tab_layout = match preset.layout.tab_layout() {
            photon_core::TabLayout::Horizontal => "Horizontal tabs",
            photon_core::TabLayout::Vertical => "Vertical tabs",
        };
        let sidebar_side = match preset.layout.sidebar_side() {
            photon_core::SidebarSide::Left => "left sidebar",
            photon_core::SidebarSide::Right => "right sidebar",
        };
        let summary = format!("{tab_layout} · {sidebar_side}");
        let is_default = current.workspace.default_preset_id == id;
        let edit = edit_preset(id.clone());
        let delete = (current.workspace.presets.len() > 1).then(|| {
            button(
                (ElementId::from("settings-delete-layout-preset"), id.clone()),
                "Delete",
                ButtonKind::Destructive,
                true,
                ButtonSize::Regular,
                palette,
                delete_preset(id.clone()),
            )
            .aria_label(format!("Delete {preset_name}"))
            .into_any_element()
        });
        h_stack()
            .id((ElementId::from("settings-layout-preset"), id.clone()))
            .items_center()
            .justify_between()
            .gap(px(metrics::INTERNAL_PAGE_GROUP_GAP))
            .w_full()
            .p(px(metrics::INTERNAL_PAGE_GROUP_GAP))
            .rounded(px(metrics::control_radius()))
            .border_1()
            .border_color(rgba(palette.control_border))
            .child(
                v_stack()
                    .flex_1()
                    .min_w_0()
                    .gap(px(metrics::MENU_ITEM_GAP))
                    .child(
                        h_stack()
                            .w_full()
                            .items_center()
                            .gap(px(metrics::MENU_ITEM_GAP))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(gpui::rgb(palette.text_primary))
                                    .child(preset_name.clone()),
                            )
                            .when(is_default, |row| {
                                row.child(secondary_text("Profile default", palette))
                            }),
                    )
                    .child(secondary_text(summary, palette)),
            )
            .child(
                h_stack()
                    .items_center()
                    .gap(px(metrics::BUTTON_GAP))
                    .child(
                        button(
                            (ElementId::from("settings-edit-layout-preset"), id.clone()),
                            "Edit",
                            ButtonKind::Secondary,
                            true,
                            ButtonSize::Regular,
                            palette,
                            edit,
                        )
                        .aria_label(format!("Edit {preset_name}")),
                    )
                    .children(delete),
            )
    });
    let remaining_preset_note = (current.workspace.presets.len() == 1).then(|| {
        secondary_text(
            "Keep at least one preset so every space has a layout.",
            palette,
        )
        .into_any_element()
    });
    let profile_layout = v_stack()
        .gap(px(metrics::MENU_ITEM_GAP))
        .child(form_field(
            "Profile layout",
            h_stack()
                .items_center()
                .gap(px(metrics::BUTTON_GAP))
                .child(dropdown(
                    "settings-default-layout",
                    "Profile layout",
                    default_preset_choices,
                    palette,
                ))
                .child(button(
                    "settings-create-layout-preset",
                    "New preset…",
                    ButtonKind::Secondary,
                    true,
                    ButtonSize::Regular,
                    palette,
                    create_preset,
                )),
            palette,
        ))
        .child(secondary_text(
            "The default layout for spaces that follow the profile.",
            palette,
        ));
    let space_layout = v_stack()
        .gap(px(metrics::MENU_ITEM_GAP))
        .child(form_field(
            "Layout for this space",
            dropdown(
                "settings-space-layout",
                "Layout for this space",
                space_layout_choices,
                palette,
            ),
            palette,
        ))
        .child(secondary_text(
            "Choose a saved preset for this space or let it follow the profile default. Each space keeps its own accent and window tint.",
            palette,
        ));

    let tint_picker = super::super::super::theme_picker::theme_picker(
        ("settings-theme-picker", active_space_id),
        active_space.color,
        super::super::super::theme::space_colors(active_space),
        Rc::new(move |color, colors, cx| {
            Settings::update(cx, |settings| {
                if let Some(space) = settings.spaces.get_mut(active_space_id) {
                    space.color = color;
                    space.set_colors(colors);
                }
            })
        }),
    );

    let appearance_details = v_stack()
        .w_full()
        .max_w(px(metrics::INTERNAL_PAGE_MAX_WIDTH))
        .gap(px(metrics::INTERNAL_PAGE_SECTION_GAP))
        .child(
            v_stack()
                .gap(px(metrics::MENU_ITEM_GAP))
                .child(form_field(
                    "Window background",
                    choices(
                        "settings-window-background-style",
                        "Window background",
                        window_background_styles,
                        palette,
                    ),
                    palette,
                ))
                .child(secondary_text(
                    "Choose Liquid Glass or the native blurred window material.",
                    palette,
                )),
        )
        .child(
            v_stack()
                .gap(px(metrics::MENU_ITEM_GAP))
                .child(form_field(
                    "Transparency",
                    choices(
                        "settings-transparency",
                        "Transparency",
                        transparencies,
                        palette,
                    ),
                    palette,
                ))
                .child(secondary_text(
                    "How much of the desktop shows through tabs, toolbars, menus and pages.",
                    palette,
                )),
        )
        .child(
            v_stack()
                .gap(px(metrics::INTERNAL_PAGE_GROUP_GAP))
                .child(profile_layout)
                .child(space_layout),
        )
        .child(
            v_stack()
                .gap(px(metrics::INTERNAL_PAGE_GROUP_GAP))
                .child(label("Presets", palette))
                .children(presets)
                .children(remaining_preset_note),
        );

    v_stack()
        .w_full()
        .gap(px(metrics::INTERNAL_PAGE_SECTION_GAP))
        .child(tint_picker)
        .child(appearance_details)
}
