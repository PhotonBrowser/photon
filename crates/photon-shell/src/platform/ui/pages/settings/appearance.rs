//! Appearance settings for the profile and the active space.

use gpui::{App, ElementId, FontWeight, Role, div, prelude::*, px, rgb, rgb_to_hsla, rgba};
use photon_core::{ThemeMode, Transparency, WindowBackgroundStyle};
use std::rc::Rc;

use super::super::super::button::{ButtonKind, ButtonSize, button};
use super::super::super::controls::{Choice, Pressable, choices, intensity_slider, toggled};
use super::super::super::dropdown::dropdown;
use super::super::super::layout::{form_field, h_stack, v_stack};
use super::super::super::metrics;
use super::super::super::settings::Settings;
use super::super::super::theme::{
    AccentColor, ThemeColors, accent_swatch, accent_swatch_mark, theme_preview_palettes,
};
use super::super::super::workspace_layout::WorkspaceLayout;
use super::super::ClickHandler;
use super::super::change;
use super::super::layout::{label, secondary_text};

pub(super) fn settings(
    palette: ThemeColors,
    cx: &App,
    create_preset: ClickHandler,
    edit_preset: Rc<dyn Fn(String) -> ClickHandler>,
    delete_preset: Rc<dyn Fn(String) -> ClickHandler>,
) -> impl IntoElement {
    let current = Settings::get(cx);
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
    let active_accent = AccentColor::from_window_color(active_space.color);
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

    let theme_modes = [
        (ThemeMode::Light, "Light"),
        (ThemeMode::Dark, "Dark"),
        (ThemeMode::System, "System"),
    ];
    let theme_selector = h_stack()
        .id("settings-theme")
        .role(Role::RadioGroup)
        .aria_label("Theme")
        .w_full()
        .gap(px(metrics::INTERNAL_PAGE_GROUP_GAP))
        .children(theme_modes.into_iter().map(|(mode, label)| {
            theme_card(
                mode,
                label,
                current.theme == mode,
                active_accent,
                palette,
                change(move |settings| settings.theme = mode),
            )
        }));

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
        .child(form_field(
            "Grain",
            intensity_slider(
                "settings-background-noise",
                "Grain intensity",
                noise_intensity_percent(current.background_noise_intensity),
                NOISE_SLIDER_PERCENT_MAX,
                NOISE_SLIDER_PERCENT_STEP,
                palette,
                |intensity, cx| {
                    Settings::update(cx, |settings| {
                        settings.background_noise_intensity =
                            noise_intensity_from_percent(intensity);
                    });
                },
            ),
            palette,
        ))
        .child(secondary_text(
            "Adjust static grain on the window background.",
            palette,
        ))
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
        .child(theme_and_accent_panel(
            theme_selector,
            active_accent,
            active_space_id,
            palette,
        ))
        .child(appearance_details)
}

fn theme_and_accent_panel(
    theme_selector: impl IntoElement,
    accent: AccentColor,
    active_space_id: photon_core::SpaceId,
    palette: ThemeColors,
) -> impl IntoElement {
    v_stack()
        .w_full()
        .rounded(px(metrics::theme_accent_panel_radius()))
        .border_1()
        .border_color(rgba(palette.menu_border))
        .bg(rgba(palette.menu_surface))
        .child(
            v_stack()
                .w_full()
                .gap(px(metrics::MENU_ITEM_GAP))
                .p(px(metrics::ACCENT_SECTION_PADDING))
                .child(label("Theme", palette))
                .child(theme_selector),
        )
        .child(
            div()
                .w_full()
                .h(px(metrics::THEME_ACCENT_DIVIDER_HEIGHT))
                .bg(rgba(palette.menu_border)),
        )
        .child(accent_color_section(accent, active_space_id, palette))
}

fn theme_card(
    mode: ThemeMode,
    label: &'static str,
    chosen: bool,
    accent: AccentColor,
    palette: ThemeColors,
    on_choose: ClickHandler,
) -> impl IntoElement {
    let (sidebar, page) = theme_preview_palettes(mode, accent);
    v_stack()
        .id((ElementId::from("settings-theme-option"), label))
        .role(Role::RadioButton)
        .aria_label(label)
        .aria_toggled(toggled(chosen))
        .tab_index(0)
        .flex_1()
        .min_w_0()
        .items_center()
        .gap(px(metrics::MENU_ITEM_GAP))
        .focus_visible(|style| style.bg(rgba(palette.hover_surface)))
        .pressable()
        .on_click(on_choose)
        .child(
            v_stack()
                .w_full()
                .p(px(metrics::THEME_PREVIEW_SELECTION_INSET))
                .rounded(px(metrics::theme_preview_selection_radius()))
                .border_2()
                .border_color(if chosen {
                    rgb_to_hsla(rgb(palette.text_primary))
                } else {
                    gpui::transparent_black()
                })
                .child(theme_thumbnail(sidebar, page, label)),
        )
        .child(
            div()
                .text_size(px(metrics::TAB_FONT_SIZE))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(palette.text_secondary))
                .child(label),
        )
}

fn theme_thumbnail(
    sidebar: ThemeColors,
    page: ThemeColors,
    label: &'static str,
) -> impl IntoElement {
    h_stack()
        .id((ElementId::from("settings-theme-thumbnail"), label))
        .aria_hidden()
        .w_full()
        .h(px(metrics::THEME_PREVIEW_HEIGHT))
        .gap(px(metrics::THEME_PREVIEW_GAP))
        .p(px(metrics::THEME_PREVIEW_INSET))
        .rounded(px(metrics::theme_preview_radius()))
        .bg(rgba(sidebar.menu_surface))
        .child(
            v_stack()
                .flex_shrink_0()
                .w(px(metrics::THEME_PREVIEW_SIDEBAR_WIDTH))
                .h_full()
                .gap(px(metrics::MENU_ITEM_GAP))
                .p(px(metrics::THEME_PREVIEW_INSET))
                .rounded(px(metrics::control_radius()))
                .bg(rgba(sidebar.menu_surface))
                .child(preview_line(
                    metrics::THEME_PREVIEW_TITLE_WIDTH / 3.0,
                    metrics::THEME_PREVIEW_LINE_HEIGHT,
                    rgb(sidebar.text_secondary),
                ))
                .child(
                    v_stack()
                        .w_full()
                        .gap(px(metrics::MENU_ITEM_GAP))
                        .p(px(metrics::THEME_PREVIEW_INSET))
                        .rounded(px(metrics::control_radius()))
                        .bg(rgba(sidebar.selected_surface))
                        .child(preview_line(
                            metrics::THEME_PREVIEW_TITLE_WIDTH / 2.0,
                            metrics::THEME_PREVIEW_LINE_HEIGHT,
                            rgb(sidebar.text_secondary),
                        ))
                        .child(preview_line(
                            metrics::THEME_PREVIEW_TITLE_WIDTH / 2.0,
                            metrics::THEME_PREVIEW_LINE_HEIGHT,
                            rgb(sidebar.text_secondary),
                        )),
                )
                .child(preview_line(
                    metrics::THEME_PREVIEW_TITLE_WIDTH / 2.0,
                    metrics::THEME_PREVIEW_LINE_HEIGHT,
                    rgb(sidebar.text_secondary),
                )),
        )
        .child(
            v_stack()
                .flex_1()
                .min_w_0()
                .h_full()
                .items_center()
                .justify_center()
                .gap(px(metrics::MENU_ITEM_GAP))
                .p(px(metrics::THEME_PREVIEW_INSET))
                .rounded(px(metrics::control_radius()))
                .bg(rgb(page.page_background))
                .child(preview_line(
                    metrics::THEME_PREVIEW_TITLE_WIDTH,
                    metrics::THEME_PREVIEW_LINE_HEIGHT,
                    rgb(page.text_secondary),
                ))
                .child(preview_line(
                    metrics::THEME_PREVIEW_TITLE_WIDTH * 1.5,
                    metrics::THEME_PREVIEW_LINE_HEIGHT,
                    rgba(page.menu_border),
                ))
                .child(preview_line(
                    metrics::THEME_PREVIEW_TITLE_WIDTH * 1.5,
                    metrics::THEME_PREVIEW_LINE_HEIGHT,
                    rgba(page.menu_border),
                )),
        )
}

fn preview_line(width: f32, height: f32, color: impl Into<gpui::Background>) -> gpui::Div {
    div().w(px(width)).h(px(height)).rounded_full().bg(color)
}

fn accent_color_section(
    accent: AccentColor,
    active_space_id: photon_core::SpaceId,
    palette: ThemeColors,
) -> impl IntoElement {
    let swatches = AccentColor::ALL.into_iter().map(|color| {
        let chosen = accent == color;
        h_stack()
            .id((
                ElementId::from("settings-accent-color-option"),
                color.name(),
            ))
            .role(Role::RadioButton)
            .aria_label(color.name())
            .aria_toggled(toggled(chosen))
            .tab_index(0)
            .size(px(metrics::ACCENT_SWATCH_RING_SIZE))
            .items_center()
            .justify_center()
            .rounded_full()
            .border_2()
            .border_color(if chosen {
                rgb_to_hsla(rgb(palette.text_primary))
            } else {
                gpui::transparent_black()
            })
            .focus_visible(|style| style.border_color(rgb(palette.chosen)))
            .pressable()
            .on_click(change(move |settings| {
                if let Some(space) = settings.spaces.get_mut(active_space_id) {
                    space.color = color.window_color();
                }
            }))
            .child(
                h_stack()
                    .size(px(metrics::ACCENT_SWATCH_SIZE))
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .border_1()
                    .border_color(rgba(palette.control_border))
                    .bg(rgb(accent_swatch(color)))
                    .when(chosen, |swatch| {
                        swatch.child(
                            div()
                                .text_size(px(metrics::CHECKBOX_ICON_SIZE))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(accent_swatch_mark()))
                                .child("✓"),
                        )
                    }),
            )
    });

    h_stack()
        .w_full()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap(px(metrics::INTERNAL_PAGE_GROUP_GAP))
        .p(px(metrics::ACCENT_SECTION_PADDING))
        .child(
            v_stack()
                .flex_1()
                .min_w_0()
                .gap(px(metrics::FORM_FIELD_GAP))
                .child(label("Accent color", palette))
                .child(secondary_text(
                    "A little color for your controls and selections.",
                    palette,
                )),
        )
        .child(
            h_stack()
                .id("settings-accent-color")
                .role(Role::RadioGroup)
                .aria_label("Accent color")
                .flex_wrap()
                .items_center()
                .justify_end()
                .gap(px(metrics::ACCENT_SWATCH_GAP))
                .children(swatches),
        )
}

const NOISE_SLIDER_PERCENT_MAX: u8 = 100;
const NOISE_SLIDER_PERCENT_STEP: u8 =
    NOISE_SLIDER_PERCENT_MAX / photon_core::BACKGROUND_NOISE_INTENSITY_MAX;

/// Shows the stored 0–20 strength on the settings slider's 0–100% scale.
fn noise_intensity_percent(intensity: u8) -> u8 {
    (u16::from(intensity.min(photon_core::BACKGROUND_NOISE_INTENSITY_MAX))
        * u16::from(NOISE_SLIDER_PERCENT_MAX)
        / u16::from(photon_core::BACKGROUND_NOISE_INTENSITY_MAX)) as u8
}

/// Maps the slider's displayed percentage back to the stored 0–20 strength.
fn noise_intensity_from_percent(percent: u8) -> u8 {
    ((u16::from(percent.min(NOISE_SLIDER_PERCENT_MAX))
        * u16::from(photon_core::BACKGROUND_NOISE_INTENSITY_MAX)
        + u16::from(NOISE_SLIDER_PERCENT_MAX / 2))
        / u16::from(NOISE_SLIDER_PERCENT_MAX)) as u8
}
