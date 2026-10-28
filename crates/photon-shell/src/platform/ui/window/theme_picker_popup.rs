//! Opens the shared theme picker as a sidebar-anchored browser popover.

use gpui::{AnyElement, Context, KeyDownEvent, MouseButton, Role, Window, div, prelude::*, px};
use photon_core::{DockRegion, SpaceId};

use super::super::controls::OwnsClicks;
use super::super::motion::{AnimateIn, theme_picker_entrance};
use super::super::settings::Settings;
use super::super::theme;
use super::super::{metrics, theme_picker};
use super::BrowserWindow;

impl BrowserWindow {
    pub(super) fn show_theme_picker(
        &mut self,
        space: SpaceId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if Settings::get(cx).spaces.get(space).is_none() {
            return;
        }
        self.theme_picker_space = Some(space);
        window.focus(&self.root_focus, cx);
        cx.notify();
    }

    pub(super) fn dismiss_theme_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.theme_picker_space.take().is_some() {
            if let Some(active_tab) = self.active_tab {
                self.activate_tab(active_tab, true, window, cx);
            } else {
                self.omnibox
                    .update(cx, |omnibox, cx| omnibox.focus(window, cx));
            }
            cx.notify();
        }
    }

    /// The theme editor floats beside the sidebar and closes on outside click or Escape.
    pub(super) fn theme_picker_popup_overlay(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let requested = self
            .theme_picker_space
            .filter(|id| Settings::get(cx).spaces.get(*id).is_some());
        if requested.is_none() {
            self.theme_picker_space = None;
        }
        let animations = Settings::motion(cx);
        let picker_motion = theme_picker_entrance(metrics::PICKER_WIDTH);
        let (space_id, transition) =
            self.theme_picker_presence
                .sync(requested, picker_motion, &animations, cx)?;
        let space = Settings::get(cx).spaces.get(space_id)?.clone();
        let (left, top) = self.picker_popup_position(window, cx);
        let dialog_label = format!("Edit theme for {}", space.name);
        #[cfg(target_os = "macos")]
        let glass = self.theme_picker_glass.clone().map(|glass| {
            glass
                .corner_radius(px(metrics::theme_accent_panel_radius()))
                // No tint beneath the glass: it refracts the page itself, and
                // the picker paints the theme's tint over it.
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                // The native view follows only its own style, so it takes the
                // panel's motion itself rather than inheriting a painted scale.
                .animate_with_offset_stable_id(
                    "window-theme-picker-glass-motion",
                    picker_motion,
                    transition,
                    &animations,
                    (0.0, 0.0),
                )
        });
        #[cfg(target_os = "macos")]
        let over_glass = glass.is_some();
        #[cfg(not(target_os = "macos"))]
        let over_glass = false;
        let picker = theme_picker::theme_picker(
            ("window-theme-picker", space_id),
            space.color,
            theme::space_colors(&space),
            Some(space_id),
            theme_picker::change_space_theme(space_id),
        )
        .into_floating()
        .over_glass(over_glass);
        let dialog = div()
            .child(
                div()
                    .id("theme-picker-dialog")
                    .role(Role::Dialog)
                    .aria_label(dialog_label)
                    .owns_clicks()
                    .child(picker),
            )
            .animate_with_offset_stable_id(
                "window-theme-picker-panel-motion",
                picker_motion,
                transition,
                &animations,
                (0.0, 0.0),
            );
        let panel = div()
            .id("window-theme-picker-panel")
            .absolute()
            .left(px(left))
            .top(px(top));
        #[cfg(target_os = "macos")]
        let panel = panel.children(glass);
        let panel = panel.child(dialog);
        let mut overlay = div()
            .id("window-theme-picker-overlay")
            .absolute()
            .inset_0()
            .child(panel);
        if self.theme_picker_space.is_some() {
            overlay = overlay
                .occlude()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| this.dismiss_theme_picker(window, cx)),
                )
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(|this, _, window, cx| this.dismiss_theme_picker(window, cx)),
                )
                .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    if event.keystroke.key.as_str() == "escape"
                        && !event.keystroke.modifiers.modified()
                    {
                        cx.stop_propagation();
                        this.dismiss_theme_picker(window, cx);
                    }
                }));
        }
        Some(overlay.into_any_element())
    }

    fn picker_popup_position(&self, window: &Window, cx: &Context<Self>) -> (f32, f32) {
        let viewport = window.viewport_size();
        let viewport_width = f32::from(viewport.width);
        let viewport_height = f32::from(viewport.height);
        let width = metrics::PICKER_WIDTH;
        let height = metrics::PICKER_POPUP_HEIGHT;
        let inset = metrics::PAGE_INSET;
        let gap = metrics::PAGE_INSET + metrics::PAGE_CHROME_GAP;
        let layout = self.layout(cx);
        let region = layout.sidebar_region();
        let sidebar_width = if self.sidebar.over_page() || !self.sidebar.collapsed() {
            self.sidebar.width(cx)
        } else {
            metrics::SIDEBAR_COLLAPSED_WIDTH
        };
        let sidebar_inset = if self.sidebar.over_page() {
            metrics::SIDEBAR_REVEAL_INSET
        } else {
            0.0
        };
        let sidebar_top = layout.top_extent() * self.sidebar.chrome_shown() + sidebar_inset;
        let sidebar_bottom = layout.bottom_extent() * self.sidebar.chrome_shown() + sidebar_inset;
        let centered_top = (sidebar_top + viewport_height - sidebar_bottom - height) / 2.0;

        let (desired_left, desired_top) = match (region, self.theme_picker_anchor) {
            (Some(DockRegion::Leading), _) => (sidebar_inset + sidebar_width + gap, centered_top),
            (Some(DockRegion::Trailing), _) => (
                viewport_width - sidebar_inset - sidebar_width - width - gap,
                centered_top,
            ),
            (_, Some(anchor)) => {
                let pointer_x = f32::from(anchor.x);
                let left = if pointer_x + width + gap <= viewport_width {
                    pointer_x + gap
                } else {
                    pointer_x - width - gap
                };
                (left, f32::from(anchor.y) - height / 2.0)
            }
            _ => (
                (viewport_width - width) / 2.0,
                (viewport_height - height) / 2.0,
            ),
        };

        let max_left = (viewport_width - width - inset).max(inset);
        let max_top = (viewport_height - height - inset).max(inset);
        (
            desired_left.clamp(inset, max_left),
            desired_top.clamp(inset, max_top),
        )
    }
}
