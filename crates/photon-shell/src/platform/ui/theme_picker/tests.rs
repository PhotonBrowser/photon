use super::*;
use gpui::point;
#[gpui::test]
fn field_drag_previews_live_and_commits_only_on_release(cx: &mut gpui::TestAppContext) {
    use std::cell::Cell;
    cx.update(|cx| {
        super::super::settings::load_settings(
            None,
            photon_core::BrowserSettings::default(),
            None,
            cx,
        )
    });
    let commits = Rc::new(Cell::new(0));
    let on_commit = commits.clone();
    let (picker, cx) = cx.add_window_view(|_, cx| {
        PickerState::new(
            WindowColor::Blue,
            SpaceColors::from_colors(&[theme::preset_tint(WindowColor::Blue)]),
            Rc::new(move |_, _, _, _, _| on_commit.set(on_commit.get() + 1)),
            cx,
        )
    });
    let bounds = cx
        .debug_bounds("Theme colour: left/right hue, up/down brightness")
        .expect("field is rendered");
    let start = bounds.center();
    let end = start + point(px(35.0), px(25.0));
    cx.simulate_mouse_down(start, MouseButton::Left, gpui::Modifiers::default());
    cx.simulate_mouse_move(end, MouseButton::Left, gpui::Modifiers::default());
    cx.update(|window, cx| {
        window.simulate_next_frame(cx);
    });
    cx.update(|_, cx| {
        assert_eq!(
            AppearancePreview::get(cx).tint.unwrap().2,
            picker.read(cx).colors()
        );
        assert_eq!(
            Settings::get(cx).spaces.active().tint,
            None,
            "live feedback must not mutate saved settings"
        );
    });
    assert_eq!(commits.get(), 0);
    cx.simulate_mouse_up(end, MouseButton::Left, gpui::Modifiers::default());
    assert_eq!(commits.get(), 1);
    cx.update(|window, cx| {
        window.simulate_next_frame(cx);
        assert!(AppearancePreview::get(cx).owner.is_none());
    });
    assert_eq!(
        commits.get(),
        1,
        "queued frames must not replay a release commit"
    );
}

#[gpui::test]
fn pointer_samples_coalesce_per_frame_without_browser_settings_notifications(
    cx: &mut gpui::TestAppContext,
) {
    use std::cell::Cell;
    cx.update(|cx| {
        super::super::settings::load_settings(
            None,
            photon_core::BrowserSettings::default(),
            None,
            cx,
        )
    });
    let notifications = Rc::new(Cell::new(0));
    let observed = notifications.clone();
    let _subscription =
        cx.update(|cx| cx.observe_global::<Settings>(move |_| observed.set(observed.get() + 1)));
    let frames = Rc::new(Cell::new(0));
    let observed_frames = frames.clone();
    let _preview = cx.update(|cx| {
        cx.observe_global::<AppearancePreview>(move |_| {
            observed_frames.set(observed_frames.get() + 1)
        })
    });
    let (picker, cx) = cx.add_window_view(|_, cx| {
        PickerState::new(
            WindowColor::Blue,
            SpaceColors::from_colors(&[theme::preset_tint(WindowColor::Blue)]),
            Rc::new(|_, _, _, _, _| {}),
            cx,
        )
    });
    cx.update(|window, cx| {
        for sample in 0..1_000 {
            let x = 0.2 + 0.5 * sample as f32 / 999.0;
            picker.update(cx, |picker, cx| {
                picker.set_tint(tint_at(x, 0.5), false, cx);
                picker.dirty_tint = true;
                picker.changed(window, cx);
            });
        }
        window.simulate_next_frame(cx);
    });
    assert_eq!(frames.get(), 1, "one preview publication per frame");
    assert_eq!(
        notifications.get(),
        0,
        "dragging must not trigger browser-policy observers"
    );
    cx.update(|_, cx| {
        assert_eq!(
            AppearancePreview::get(cx).tint.unwrap().2,
            SpaceColors::from_colors(&[tint_at(0.7, 0.5)])
        )
    });
}

#[gpui::test]
fn intensity_changes_alpha_in_realtime_without_whitening_the_colour(cx: &mut gpui::TestAppContext) {
    let mut settings = photon_core::BrowserSettings::default();
    let id = settings.spaces.active().id;
    settings.spaces.get_mut(id).unwrap().tint = Some(SpaceTint::default());
    settings.transparency_percent = Some(50);
    cx.update(|cx| super::super::settings::load_settings(None, settings, None, cx));
    let (picker, cx) = cx.add_window_view(|_, cx| {
        PickerState::new(
            WindowColor::Blue,
            SpaceColors::from_colors(&[SpaceTint::default()]),
            super::change_space_theme(id),
            cx,
        )
    });
    let bounds = cx.debug_bounds("Intensity").expect("slider is rendered");
    cx.simulate_mouse_down(
        bounds.center(),
        MouseButton::Left,
        gpui::Modifiers::default(),
    );
    cx.simulate_mouse_move(
        point(bounds.right(), bounds.center().y),
        MouseButton::Left,
        gpui::Modifiers::default(),
    );
    cx.update(|window, cx| {
        window.simulate_next_frame(cx);
        let palette = theme::palette(window, cx);
        assert_eq!(
            palette.window_tint & 0xff,
            255,
            "maximum intensity makes the tint opaque"
        );
        assert_eq!(
            Settings::get(cx).transparency_percent,
            Some(50),
            "profile isn't changed while dragging"
        );
        assert_eq!(
            picker.read(cx).colors(),
            SpaceColors::from_colors(&[SpaceTint::default()]),
            "transparency preserves the chosen colour"
        );
    });
    cx.simulate_mouse_up(
        point(bounds.right(), bounds.center().y),
        MouseButton::Left,
        gpui::Modifiers::default(),
    );
    cx.update(|_, cx| assert_eq!(Settings::get(cx).transparency_percent, Some(0)));
    cx.simulate_keystrokes("home left");
    cx.update(|window, cx| {
        assert_eq!(Settings::get(cx).transparency_percent, Some(100));
        assert_eq!(theme::palette(window, cx).window_tint & 0xff, 0);
    });
}

#[test]
fn field_round_trips_presets() {
    for color in WindowColor::ALL
        .into_iter()
        .filter(|color| *color != WindowColor::System)
    {
        let tint = theme::preset_tint(color);
        let (x, y) = field_position(tint);
        let restored = tint_at(x, y);
        assert!((i32::from(restored.hue) - i32::from(tint.hue)).abs() <= 1);
        assert_eq!(restored.lightness, tint.lightness);
    }
}
#[test]
fn wave_haptics_cross_extrema_in_both_directions() {
    assert_eq!(wave_detent(4), 0);
    assert_eq!(wave_detent(5), 1);
    assert_eq!(wave_detent(14), 1);
    assert_eq!(wave_detent(15), 2);
    assert_eq!(wave_detent(100), 10);
}
#[test]
fn dial_tracks_clockwise_and_ignores_center() {
    assert_eq!(noise_at(0.5, 0.0), Some(0));
    assert_eq!(noise_at(1.0, 0.5), Some(5));
    assert_eq!(noise_at(0.5, 1.0), Some(10));
    assert_eq!(noise_at(0.0, 0.5), Some(15));
    assert_eq!(noise_at(0.5, 0.5), None);
}

#[gpui::test]
fn add_and_remove_buttons_edit_the_gradient_and_allow_zero_colours(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        super::super::settings::load_settings(
            None,
            photon_core::BrowserSettings::default(),
            None,
            cx,
        )
    });
    let (picker, cx) = cx.add_window_view(|_, cx| {
        PickerState::new(
            WindowColor::Blue,
            SpaceColors::from_colors(&[SpaceTint::default()]),
            Rc::new(|color, colors, _, _, cx| {
                Settings::update(cx, |settings| {
                    let id = settings.spaces.active().id;
                    let space = settings.spaces.get_mut(id).unwrap();
                    space.color = color;
                    space.set_colors(colors);
                });
            }),
            cx,
        )
    });
    let add = cx
        .debug_bounds("Add colour")
        .expect("add colour is rendered")
        .center();
    let remove = cx
        .debug_bounds("Remove colour")
        .expect("remove colour is rendered")
        .center();
    for expected in [2, 3, 3] {
        cx.simulate_click(add, gpui::Modifiers::default());
        cx.update(|_, cx| {
            assert_eq!(picker.read(cx).colors().colors().len(), expected);
            assert_eq!(
                Settings::get(cx)
                    .spaces
                    .active()
                    .colors
                    .unwrap()
                    .colors()
                    .len(),
                expected
            );
        });
    }
    for expected in [2, 1, 0, 0] {
        cx.simulate_click(remove, gpui::Modifiers::default());
        cx.update(|_, cx| {
            assert_eq!(picker.read(cx).colors().colors().len(), expected);
            assert_eq!(Settings::get(cx).spaces.active().gradient, expected > 1);
        });
    }
}

#[gpui::test]
fn secondary_selection_becomes_primary_and_dragging_moves_the_palette_together(
    cx: &mut gpui::TestAppContext,
) {
    let colors =
        SpaceColors::from_colors(&[tint_at(0.2, 0.4), tint_at(0.7, 0.7), tint_at(0.4, 0.6)]);
    let mut settings = photon_core::BrowserSettings::default();
    let id = settings.spaces.active().id;
    settings.spaces.get_mut(id).unwrap().set_colors(colors);
    cx.update(|cx| super::super::settings::load_settings(None, settings, None, cx));
    let (picker, cx) = cx.add_window_view(|_, cx| {
        PickerState::new(WindowColor::Blue, colors, Rc::new(save_colors), cx)
    });
    let bounds = cx
        .debug_bounds("Theme colour: left/right hue, up/down brightness")
        .unwrap();
    let radius = metrics::PICKER_HANDLE_RADIUS;
    let at = |x: f32, y: f32| {
        bounds.origin
            + point(
                px(radius + x * (f32::from(bounds.size.width) - radius * 2.0)),
                px(radius + y * (f32::from(bounds.size.height) - radius * 2.0)),
            )
    };
    let (x, y) = cx.update(|_, cx| {
        let state = picker.read(cx);
        let handle = state.handles.iter().find(|handle| handle.id == 1).unwrap();
        handle.position(Instant::now(), &Settings::motion(cx))
    });
    let start = at(x, y);
    let end = at(x + 0.1, y + 0.05);
    cx.simulate_mouse_down(start, MouseButton::Left, gpui::Modifiers::default());
    cx.update(|_, cx| {
        let state = picker.read(cx);
        assert_eq!(state.colors().primary_index(), 1);
        assert_eq!(
            state.colors().colors(),
            colors.colors(),
            "selecting a primary must preserve each colour's identity"
        );
    });
    cx.simulate_mouse_move(end, MouseButton::Left, gpui::Modifiers::default());
    cx.update(|window, cx| {
        window.simulate_next_frame(cx);
        let state = picker.read(cx);
        let updated = state.colors();
        assert_eq!(state.selected, 1);
        assert_eq!(updated.primary_index(), 1);
        let before = tint_at(x, y);
        let after = tint_at(x + 0.1, y + 0.05);
        let hue_delta = (i32::from(after.hue) - i32::from(before.hue)).rem_euclid(360);
        let lightness_delta = i32::from(after.lightness) - i32::from(before.lightness);
        for (original, changed) in colors.colors().iter().zip(updated.colors()) {
            assert_eq!(
                i32::from(changed.hue),
                (i32::from(original.hue) + hue_delta).rem_euclid(360),
                "all colours rotate by the same hue delta"
            );
            assert_eq!(changed.saturation, original.saturation);
            assert_eq!(
                i32::from(changed.lightness),
                (i32::from(original.lightness) + lightness_delta).clamp(0, 100)
            );
        }
        let primary = state
            .handles
            .iter()
            .find(|handle| handle.id == state.selected)
            .unwrap();
        assert!(
            (primary.x.target() - (x + 0.1)).abs() < 0.001,
            "primary spring targets the pointer"
        );
        assert!(
            primary.x.is_running(),
            "primary follows the pointer using an interruptible spring"
        );
        assert!(
            state
                .handles
                .iter()
                .any(|handle| handle.id != state.selected && handle.is_animating(Instant::now())),
            "satellites follow using springs"
        );
        let palette = theme::palette(window, cx);
        assert_eq!(
            palette.window_tint_middle.unwrap() >> 8,
            theme::tint_swatch(updated.primary().unwrap()),
            "the selected colour leads the gradient"
        );
    });
    cx.simulate_mouse_up(end, MouseButton::Left, gpui::Modifiers::default());
    cx.update(|_, cx| {
        let saved = Settings::get(cx).spaces.active();
        assert_eq!(saved.colors.unwrap().primary_index(), 1);
        assert_eq!(saved.tint, saved.colors.unwrap().primary());
    });
}

#[gpui::test]
fn keyboard_selection_saves_the_primary_without_recolouring_the_palette(
    cx: &mut gpui::TestAppContext,
) {
    let colors =
        SpaceColors::from_colors(&[tint_at(0.3, 0.3), tint_at(0.5, 0.6), tint_at(0.7, 0.5)])
            .with_primary(1);
    let mut settings = photon_core::BrowserSettings::default();
    let id = settings.spaces.active().id;
    settings.spaces.get_mut(id).unwrap().set_colors(colors);
    cx.update(|cx| super::super::settings::load_settings(None, settings, None, cx));
    let (picker, cx) = cx.add_window_view(|_, cx| {
        PickerState::new(WindowColor::Blue, colors, Rc::new(save_colors), cx)
    });
    cx.update(|window, cx| {
        assert_eq!(picker.read(cx).selected, 1);
        let focus = picker.read(cx).focus[0].clone();
        window.focus(&focus, cx);
    });
    cx.simulate_keystrokes("]");
    cx.update(|_, cx| {
        let saved = Settings::get(cx).spaces.active().colors.unwrap();
        assert_eq!(saved.primary_index(), 2);
        assert_eq!(saved.colors(), colors.colors());
    });
}

#[test]
fn handle_springs_overshoot_and_reduced_motion_snaps() {
    use super::super::motion::{MotionConfig, MotionTimings, THEME_PICKER_SPRING};
    use super::handles::ColorHandle;
    use std::time::Duration;
    let duration = Duration::from_millis(200);
    let config = MotionConfig {
        enabled: true,
        reduce_motion: false,
        effect_strength: 1.0,
        timings: MotionTimings {
            quick: duration,
            standard: duration,
            gentle: duration,
        },
    };
    let mut handle = ColorHandle::new(0, SpaceTint::default(), true);
    handle.scale.jump_to(0.0);
    let start = Instant::now();
    handle
        .scale
        .animate_to_at(1.0, duration, THEME_PICKER_SPRING, start);
    assert!(
        handle.scale.value_at(start + duration) > 1.0,
        "colour handles should visibly bounce past their target"
    );
    handle.scale.animate_to(
        0.6,
        Speed::Standard,
        &MotionConfig {
            reduce_motion: true,
            ..config
        },
    );
    assert_eq!(handle.scale.value(), 0.6);
    assert!(!handle.scale.is_running());
}

#[test]
fn gradient_colours_round_trip_the_profile_toml() {
    let colors =
        SpaceColors::from_colors(&[tint_at(0.2, 0.4), tint_at(0.7, 0.7), tint_at(0.4, 0.6)])
            .with_primary(2);
    let mut settings = photon_core::BrowserSettings::default();
    let id = settings.spaces.active().id;
    settings.spaces.get_mut(id).unwrap().set_colors(colors);
    let saved = toml::to_string(&settings).expect("gradient palettes serialize in a profile");
    let restored: photon_core::BrowserSettings =
        toml::from_str(&saved).expect("gradient profiles load again");
    assert_eq!(restored.spaces.active().colors, Some(colors));
}

fn save_colors(
    color: WindowColor,
    colors: SpaceColors,
    _transparency: Option<u8>,
    _noise: Option<u8>,
    cx: &mut App,
) {
    Settings::update(cx, |settings| {
        let id = settings.spaces.active().id;
        let space = settings.spaces.get_mut(id).unwrap();
        space.color = color;
        space.set_colors(colors);
    });
}

#[gpui::test]
fn removing_the_last_colour_restores_the_default_and_subtle_transparency(
    cx: &mut gpui::TestAppContext,
) {
    let colors = SpaceColors::from_colors(&[tint_at(0.7, 0.4)]);
    let mut settings = photon_core::BrowserSettings {
        transparency_percent: Some(80),
        ..Default::default()
    };
    let id = settings.spaces.active().id;
    settings.spaces.get_mut(id).unwrap().set_colors(colors);
    cx.update(|cx| super::super::settings::load_settings(None, settings, None, cx));
    let (picker, cx) = cx.add_window_view(|_, cx| {
        PickerState::new(
            WindowColor::Purple,
            colors,
            super::change_space_theme(id),
            cx,
        )
    });
    let remove = cx.debug_bounds("Remove colour").unwrap().center();
    cx.simulate_click(remove, gpui::Modifiers::default());
    cx.update(|window, cx| {
        assert!(picker.read(cx).colors().colors().is_empty());
        let settings = Settings::get(cx);
        assert_eq!(settings.spaces.active().color, WindowColor::System);
        assert!(settings.spaces.active().tint.is_none());
        assert_eq!(settings.transparency, photon_core::Transparency::Subtle);
        assert_eq!(settings.transparency_percent, None);
        let palette = theme::palette(window, cx);
        assert_eq!(
            palette.window_tint >> 8,
            theme::tint_swatch(theme::default_tint(window.appearance()))
        );
        assert_eq!(
            AppearancePreview::transparency(cx),
            AppearancePreview::subtle_transparency()
        );
    });
    assert!(cx.debug_bounds("theme-picker-empty-prompt").is_some());
}

#[gpui::test]
fn empty_controls_do_nothing_until_a_field_click_creates_the_first_colour(
    cx: &mut gpui::TestAppContext,
) {
    cx.update(|cx| {
        super::super::settings::load_settings(
            None,
            photon_core::BrowserSettings::default(),
            None,
            cx,
        )
    });
    let (picker, cx) = cx.add_window_view(|_, cx| {
        PickerState::new(
            WindowColor::System,
            SpaceColors::default(),
            Rc::new(save_colors),
            cx,
        )
    });
    for selector in [
        "theme-mode-Dark",
        "theme-preset-Blue",
        "Intensity",
        "Grain intensity",
    ] {
        let bounds = cx
            .debug_bounds(selector)
            .expect("disabled control remains visible");
        cx.simulate_click(bounds.center(), gpui::Modifiers::default());
    }
    cx.update(|_, cx| {
        assert!(picker.read(cx).colors().colors().is_empty());
        assert_eq!(Settings::get(cx).theme, ThemeMode::System);
        assert_eq!(
            Settings::get(cx).background_noise_intensity,
            photon_core::BrowserSettings::default().background_noise_intensity
        );
    });
    let field = cx
        .debug_bounds("Theme colour: left/right hue, up/down brightness")
        .unwrap();
    let at = field.origin + point(px(110.0), px(190.0));
    let (x, y) = field_coordinates(at, field);
    cx.simulate_mouse_down(at, MouseButton::Left, gpui::Modifiers::default());
    cx.update(|window, cx| {
        window.simulate_next_frame(cx);
        assert_eq!(picker.read(cx).colors().colors(), &[tint_at(x, y)]);
        assert_eq!(
            AppearancePreview::get(cx).tint.unwrap().2.colors(),
            &[tint_at(x, y)]
        );
    });
    cx.simulate_mouse_up(at, MouseButton::Left, gpui::Modifiers::default());
    cx.update(|_, cx| {
        assert_eq!(
            Settings::get(cx).spaces.active().colors.unwrap().colors(),
            &[tint_at(x, y)]
        )
    });
    assert!(cx.debug_bounds("theme-picker-empty-prompt").is_none());
}

#[gpui::test]
fn add_button_and_keyboard_can_activate_an_empty_picker(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        super::super::settings::load_settings(
            None,
            photon_core::BrowserSettings::default(),
            None,
            cx,
        )
    });
    let (picker, cx) = cx.add_window_view(|_, cx| {
        PickerState::new(
            WindowColor::System,
            SpaceColors::default(),
            Rc::new(save_colors),
            cx,
        )
    });
    let add = cx.debug_bounds("Add colour").unwrap().center();
    cx.simulate_click(add, gpui::Modifiers::default());
    cx.update(|_, cx| assert_eq!(picker.read(cx).colors().colors().len(), 1));
    cx.update(|window, cx| {
        picker.update(cx, |picker, cx| picker.remove_color(cx));
        let focus = picker.read(cx).focus[0].clone();
        window.focus(&focus, cx);
    });
    cx.simulate_keystrokes("enter");
    cx.update(|_, cx| assert_eq!(picker.read(cx).colors().colors().len(), 1));
}

#[test]
fn empty_palette_round_trips_the_profile_toml() {
    let mut settings = photon_core::BrowserSettings::default();
    let id = settings.spaces.active().id;
    settings
        .spaces
        .get_mut(id)
        .unwrap()
        .set_colors(SpaceColors::default());
    let saved = toml::to_string(&settings).unwrap();
    let restored: photon_core::BrowserSettings = toml::from_str(&saved).unwrap();
    assert!(restored.spaces.active().colors.unwrap().colors().is_empty());
    assert_eq!(restored.spaces.active().color, WindowColor::System);
}

#[gpui::test]
fn selecting_a_preset_uses_the_swatch_colour(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        super::super::settings::load_settings(
            None,
            photon_core::BrowserSettings::default(),
            None,
            cx,
        )
    });
    let (picker, cx) = cx.add_window_view(|_, cx| {
        PickerState::new(
            WindowColor::Blue,
            SpaceColors::from_colors(&[SpaceTint::default()]),
            Rc::new(|_, _, _, _, _| {}),
            cx,
        )
    });
    for (color, selector) in WindowColor::ALL.into_iter().zip([
        "theme-preset-System",
        "theme-preset-Blue",
        "theme-preset-Purple",
        "theme-preset-Pink",
        "theme-preset-Red",
        "theme-preset-Orange",
        "theme-preset-Yellow",
        "theme-preset-Green",
        "theme-preset-Teal",
    ]) {
        let bounds = cx.debug_bounds(selector).expect("preset is rendered");
        cx.simulate_click(bounds.center(), gpui::Modifiers::default());
        cx.update(|_, cx| {
            let selected = picker.read(cx).colors().primary().unwrap();
            assert_eq!(
                theme::tint_swatch(selected),
                theme::window_color_swatch(color)
            );
        });
    }
}

#[test]
fn polar_mapping_has_cardinal_hues_and_clamps_to_the_circle() {
    for (point, hue) in [
        ((1.0, 0.5), 0),
        ((0.5, 1.0), 90),
        ((0.0, 0.5), 180),
        ((0.5, 0.0), 270),
    ] {
        let tint = tint_at(point.0, point.1);
        assert_eq!(tint.hue, hue);
        assert_eq!(tint.lightness, 100);
    }
    assert_eq!(tint_at(0.5, 0.5).lightness, 0);
    for point in [(-2.0, 0.7), (0.1, 5.0), (0.0, 0.0), (0.8, 0.4)] {
        let clamped = paint::clamp_field(point.0, point.1);
        assert!((clamped.0 - 0.5).hypot(clamped.1 - 0.5) <= 0.500001);
        assert_eq!(paint::clamp_field(clamped.0, clamped.1), clamped);
    }
}

#[test]
fn polar_positions_restore_hue_and_lightness_without_unit_drift() {
    for hue in 0..360 {
        for lightness in [1, 26, 55, 87, 100] {
            let tint = SpaceTint {
                hue,
                lightness,
                saturation: 60,
            };
            let position = field_position(tint);
            let restored = tint_at(position.0, position.1);
            assert_eq!(restored.hue, hue);
            assert_eq!(restored.lightness, lightness);
        }
    }
}

#[test]
fn preset_pages_wrap_and_grayscale_reaches_black_and_white() {
    let page = PresetPage::default();
    let mut next = page;
    for _ in 0..5 {
        next = next.next(1);
    }
    assert_eq!(next.label(), page.label());
    assert_eq!(page.next(-1).label(), "Grayscale");
    let black = PresetPage::Grayscale
        .colors(WindowColor::System)
        .primary()
        .unwrap();
    let white = PresetPage::Grayscale
        .colors(WindowColor::Teal)
        .primary()
        .unwrap();
    assert_eq!(theme::tint_swatch(black), 0x000000);
    assert_eq!(theme::tint_swatch(white), 0xffffff);
}

#[gpui::test]
fn preset_families_apply_their_displayed_palette(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        super::super::settings::load_settings(
            None,
            photon_core::BrowserSettings::default(),
            None,
            cx,
        )
    });
    let (picker, cx) = cx.add_window_view(|_, cx| {
        PickerState::new(
            WindowColor::Blue,
            SpaceColors::from_colors(&[SpaceTint::default()]),
            Rc::new(save_colors),
            cx,
        )
    });
    for _ in 0..5 {
        let preset = cx.debug_bounds("theme-preset-Blue").unwrap().center();
        cx.simulate_click(preset, gpui::Modifiers::default());
        cx.update(|_, cx| {
            let state = picker.read(cx);
            let expected = state.preset_page.colors(WindowColor::Blue);
            assert_eq!(state.colors(), expected);
            assert_eq!(Settings::get(cx).spaces.active().colors.unwrap(), expected);
        });
        let next = cx.debug_bounds("Next preset page").unwrap().center();
        cx.simulate_click(next, gpui::Modifiers::default());
    }
}

#[gpui::test]
fn harmony_cycles_preserve_the_primary_and_keep_shared_brightness(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        super::super::settings::load_settings(
            None,
            photon_core::BrowserSettings::default(),
            None,
            cx,
        )
    });
    let tint = theme::preset_tint(WindowColor::Blue);
    let (picker, cx) = cx.add_window_view(|_, cx| {
        PickerState::new(
            WindowColor::Blue,
            SpaceColors::from_colors(&[tint]),
            Rc::new(save_colors),
            cx,
        )
    });
    cx.update(|_, cx| {
        picker.update(cx, |state, cx| {
            state.add_color(cx);
            assert_eq!(state.colors().primary(), Some(tint));
            assert_eq!(state.colors().colors()[1].hue, (tint.hue + 180) % 360);
            state.cycle_harmony(cx);
            assert_eq!(state.colors().colors()[1].hue, (tint.hue + 310) % 360);
            state.add_color(cx);
            let original = state.colors();
            for _ in 0..3 {
                state.cycle_harmony(cx);
            }
            assert_eq!(state.colors(), original);
            for color in state.colors().colors() {
                assert_eq!(color.lightness, tint.lightness);
                assert_eq!(color.saturation, tint.saturation);
            }
        })
    });
}

#[gpui::test]
fn right_clicking_a_colour_removes_it(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        super::super::settings::load_settings(
            None,
            photon_core::BrowserSettings::default(),
            None,
            cx,
        )
    });
    let tint = theme::preset_tint(WindowColor::Blue);
    let (picker, cx) = cx.add_window_view(|_, cx| {
        PickerState::new(
            WindowColor::Blue,
            SpaceColors::from_colors(&[tint]),
            Rc::new(save_colors),
            cx,
        )
    });
    let bounds = cx
        .debug_bounds("Theme colour: left/right hue, up/down brightness")
        .unwrap();
    let (x, y) = field_position(tint);
    let radius = metrics::PICKER_HANDLE_RADIUS;
    let at = bounds.origin
        + point(
            px(radius + x * (f32::from(bounds.size.width) - radius * 2.0)),
            px(radius + y * (f32::from(bounds.size.height) - radius * 2.0)),
        );
    cx.simulate_mouse_down(at, MouseButton::Right, gpui::Modifiers::default());
    cx.simulate_mouse_up(at, MouseButton::Right, gpui::Modifiers::default());
    cx.update(|_, cx| {
        assert!(picker.read(cx).colors().colors().is_empty());
        assert!(
            Settings::get(cx)
                .spaces
                .active()
                .colors
                .unwrap()
                .colors()
                .is_empty()
        );
    });
}

#[gpui::test]
fn picker_icon_buttons_share_the_same_size(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| {
        super::super::settings::load_settings(
            None,
            photon_core::BrowserSettings::default(),
            None,
            cx,
        )
    });
    let (_, cx) = cx.add_window_view(|_, cx| {
        PickerState::new(
            WindowColor::Blue,
            SpaceColors::from_colors(&[SpaceTint::default()]),
            Rc::new(|_, _, _, _, _| {}),
            cx,
        )
    });
    for selector in [
        "theme-mode-System",
        "theme-mode-Light",
        "theme-mode-Dark",
        "Add colour",
        "Remove colour",
        "Cycle colour harmony",
        "Previous preset page",
        "Next preset page",
    ] {
        let bounds = cx.debug_bounds(selector).expect("icon button is rendered");
        assert_eq!(
            f32::from(bounds.size.width),
            metrics::PICKER_MODE_SIZE,
            "{selector}"
        );
        assert_eq!(
            f32::from(bounds.size.height),
            metrics::PICKER_MODE_SIZE,
            "{selector}"
        );
    }
}

#[gpui::test]
fn intensity_thumb_tracks_between_whole_percentage_steps(cx: &mut gpui::TestAppContext) {
    let settings = photon_core::BrowserSettings {
        transparency_percent: Some(50),
        ..Default::default()
    };
    cx.update(|cx| super::super::settings::load_settings(None, settings, None, cx));
    let (picker, cx) = cx.add_window_view(|_, cx| {
        PickerState::new(
            WindowColor::Blue,
            SpaceColors::from_colors(&[SpaceTint::default()]),
            Rc::new(|_, _, _, _, _| {}),
            cx,
        )
    });
    let bounds = cx.debug_bounds("Intensity").unwrap();
    let inset = metrics::PICKER_THUMB_MAX_WIDTH / 2.0;
    let at = |fraction: f32| {
        point(
            bounds.left() + px(inset + fraction * (f32::from(bounds.size.width) - inset * 2.0)),
            bounds.center().y,
        )
    };
    cx.simulate_mouse_down(at(0.5), MouseButton::Left, gpui::Modifiers::default());
    for fraction in [0.501, 0.502, 0.503] {
        cx.simulate_mouse_move(at(fraction), MouseButton::Left, gpui::Modifiers::default());
        cx.update(|_, cx| {
            let state = picker.read(cx);
            assert_eq!(state.transparency, 50);
            assert!((state.intensity_position - fraction).abs() < 0.0001);
        });
    }
    cx.simulate_mouse_up(at(0.503), MouseButton::Left, gpui::Modifiers::default());
}
