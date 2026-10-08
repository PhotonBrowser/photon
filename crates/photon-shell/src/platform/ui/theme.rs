//! Semantic colors and shared measurements for the native browser shell.

use gpui::{ColorExt, Rgba, WindowAppearance, colors::Colors};
use std::{cell::Cell, rc::Rc};

/// Shared shell geometry, in logical pixels.
pub(crate) mod metrics {
    pub const TITLEBAR_HEIGHT: f32 = 36.0;
    /// Height of the native window control buttons.
    const WINDOW_CONTROLS_HEIGHT: f32 = 14.0;
    /// Top-left origin of the native window controls, vertically centered in the titlebar.
    pub const WINDOW_CONTROLS_ORIGIN: (f32, f32) =
        (12.0, (TITLEBAR_HEIGHT - WINDOW_CONTROLS_HEIGHT) / 2.0);
    /// Titlebar space reserved on each side so content stays clear of the
    /// native window controls and remains centered.
    pub const WINDOW_CONTROLS_INSET: f32 = 80.0;
    pub const TAB_HEIGHT: f32 = 28.0;
    pub const TAB_MIN_WIDTH: f32 = 72.0;
    pub const TAB_MAX_WIDTH: f32 = 220.0;
    pub const TAB_STRIP_GAP: f32 = 2.0;
    pub const TAB_STRIP_INSET: f32 = 4.0;
    pub const TAB_HORIZONTAL_PADDING: f32 = 10.0;
    pub const TAB_CLOSE_GAP: f32 = 6.0;
    pub const TAB_CLOSE_BUTTON_SIZE: f32 = 22.0;
    pub const TAB_ICON_SIZE: f32 = 12.0;
    pub const TAB_FONT_SIZE: f32 = 12.0;
    pub const TAB_RADIUS: f32 = 7.0;
    pub const TOOLBAR_HEIGHT: f32 = 30.0;
    pub const TOOLBAR_HORIZONTAL_INSET: f32 = 12.0;
    pub const TOOLBAR_CONTROL_GAP: f32 = 8.0;
    pub const TOOLBAR_BUTTON_SIZE: f32 = 28.0;
    pub const TOOLBAR_ICON_SIZE: f32 = 14.0;
    pub const WINDOW_TINT_OPACITY: f32 = 0.88;
    pub const MUTED_TEXT_OPACITY: f32 = 0.72;
    pub const DISABLED_TEXT_OPACITY: f32 = 0.42;
    pub const TAB_HOVER_OPACITY: f32 = 0.08;
    pub const CONTROL_HOVER_OPACITY: f32 = 0.12;
    pub const FOCUSED_FIELD_OPACITY: f32 = 0.08;
    pub const DIAGNOSTICS_SURFACE_OPACITY: f32 = 0.95;
    pub const MENU_BORDER_OPACITY: f32 = 0.55;
    pub const MENU_HOVER_OPACITY: f32 = 0.12;
    pub const MENU_WIDTH: f32 = 196.0;
    pub const MENU_PADDING: f32 = 4.0;
    pub const MENU_RADIUS: f32 = 8.0;
    pub const MENU_FONT_SIZE: f32 = 13.0;
    pub const MENU_ITEM_HEIGHT: f32 = 30.0;
    pub const MENU_ITEM_HORIZONTAL_PADDING: f32 = 8.0;
    pub const MENU_ITEM_RADIUS: f32 = 5.0;
    pub const MENU_ITEM_GAP: f32 = 2.0;
    pub const MENU_SECTION_INSET: f32 = 6.0;
    pub const MENU_SEPARATOR_HEIGHT: f32 = 1.0;
    pub const CRASH_ALERT_MAX_WIDTH: f32 = 360.0;
    pub const CRASH_ALERT_INSET: f32 = 16.0;
    pub const CRASH_ALERT_PADDING: f32 = 12.0;
    pub const CRASH_ALERT_GAP: f32 = 8.0;
    pub const CRASH_ALERT_TITLE_SIZE: f32 = 14.0;
    pub const OMNIBOX_HEIGHT: f32 = 28.0;
    pub const OMNIBOX_HORIZONTAL_PADDING: f32 = 10.0;
    pub const OMNIBOX_GAP: f32 = 8.0;
    pub const OMNIBOX_RADIUS: f32 = 8.0;
    pub const OMNIBOX_FONT_SIZE: f32 = 13.0;
    pub const OMNIBOX_ICON_SIZE: f32 = 13.0;
    pub const PAGE_INSET: f32 = 4.0;
    pub const WEBVIEW_CORNER_RADIUS: f32 = 12.0;
}

/// The app's explicit theme preference. `None` follows the system appearance.
#[derive(Clone, Default)]
pub(crate) struct ThemePreference(Rc<Cell<Option<WindowAppearance>>>);

impl ThemePreference {
    pub fn appearance(&self, system_appearance: WindowAppearance) -> WindowAppearance {
        self.0.get().unwrap_or(system_appearance)
    }

    pub fn set(&self, appearance: WindowAppearance) {
        self.0.set(Some(appearance));
    }
}

/// Shell colors derived from GPUI-CE's semantic default palette. Fills are
/// RGBA so the window tint composes over the native window material.
#[derive(Clone, Copy)]
pub(crate) struct Palette {
    pub window_tint: u32,
    pub page_background: u32,
    pub text: u32,
    pub text_muted: u32,
    pub text_disabled: u32,
    pub accent: u32,
    pub selection: u32,
    pub field: u32,
    pub field_focused: u32,
    pub tab_active_surface: u32,
    pub tab_hover_surface: u32,
    pub tab_control_hover: u32,
    pub diagnostics_surface: u32,
    pub diagnostics_text: u32,
    pub diagnostics_muted: u32,
    pub context_menu_surface: u32,
    pub context_menu_border: u32,
    pub context_menu_hover: u32,
}

impl Palette {
    pub fn for_appearance(appearance: WindowAppearance) -> Self {
        let colors = match appearance {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => Colors::dark(),
            WindowAppearance::Light | WindowAppearance::VibrantLight => Colors::light(),
        };
        let muted = mix_colors(
            colors.text,
            colors.background,
            metrics::MUTED_TEXT_OPACITY,
        );
        let disabled = mix_colors(
            colors.text,
            colors.background,
            metrics::DISABLED_TEXT_OPACITY,
        );

        Self {
            window_tint: to_rgba_token(colors.background.opacity(metrics::WINDOW_TINT_OPACITY)),
            page_background: to_rgb_token(colors.background),
            text: to_rgb_token(colors.text),
            text_muted: to_rgb_token(muted),
            text_disabled: to_rgb_token(disabled),
            accent: to_rgb_token(colors.selected),
            selection: to_rgba_token(colors.selected),
            field: to_rgba_token(colors.container),
            field_focused: to_rgba_token(mix_colors(
                colors.selected,
                colors.container,
                metrics::FOCUSED_FIELD_OPACITY,
            )),
            tab_active_surface: to_rgba_token(colors.container),
            tab_hover_surface: to_rgba_token(colors.selected.opacity(metrics::TAB_HOVER_OPACITY)),
            tab_control_hover: to_rgba_token(colors.text.opacity(metrics::CONTROL_HOVER_OPACITY)),
            diagnostics_surface: to_rgba_token(
                colors
                    .container
                    .opacity(metrics::DIAGNOSTICS_SURFACE_OPACITY),
            ),
            diagnostics_text: to_rgb_token(colors.text),
            diagnostics_muted: to_rgb_token(muted),
            context_menu_surface: to_rgba_token(colors.container),
            context_menu_border: to_rgba_token(colors.border.opacity(metrics::MENU_BORDER_OPACITY)),
            context_menu_hover: to_rgba_token(colors.selected.opacity(metrics::MENU_HOVER_OPACITY)),
        }
    }
}

fn mix_colors(foreground: Rgba, background: Rgba, foreground_weight: f32) -> Rgba {
    let foreground_weight = foreground_weight.clamp(0.0, 1.0);
    let mix_channel = |foreground: f32, background: f32| {
        foreground * foreground_weight + background * (1.0 - foreground_weight)
    };

    Rgba::new(
        mix_channel(foreground.color.red, background.color.red),
        mix_channel(foreground.color.green, background.color.green),
        mix_channel(foreground.color.blue, background.color.blue),
        1.0,
    )
}

fn to_rgb_token(color: Rgba) -> u32 {
    (to_channel(color.color.red) << 16)
        | (to_channel(color.color.green) << 8)
        | to_channel(color.color.blue)
}

fn to_rgba_token(color: Rgba) -> u32 {
    (to_rgb_token(color) << 8) | to_channel(color.alpha)
}

fn to_channel(channel: f32) -> u32 {
    (channel.clamp(0.0, 1.0) * 255.0).round() as u32
}
