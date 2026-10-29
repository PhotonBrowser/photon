//! Opens the shared theme picker as a sidebar-anchored browser popover.

use gpui::{AnyElement, Context, KeyDownEvent, MouseButton, Role, Window, div, prelude::*, px};
use photon_core::{DockRegion, SpaceId};
use std::rc::Rc;

use super::super::controls::OwnsClicks;
use super::super::glass::{glass, glass_available, layer};
use super::super::motion::{AnimateIn, Edge, theme_picker_entrance};
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
        #[cfg(target_os = "macos")]
        if glass_available(cx) {
            self.open_theme_picker_panel(space, window, cx);
            return;
        }
        window.focus(&self.root_focus, cx);
        cx.notify();
    }

    /// Opens the picker in its own Liquid Glass window, whose glass refracts
    /// the browser's own interface behind it, which glass inside the window
    /// cannot.
    #[cfg(target_os = "macos")]
    fn open_theme_picker_panel(&mut self, space: SpaceId, window: &Window, cx: &mut Context<Self>) {
        let (left, top, edge) = self.picker_popup_position(window, cx);
        let entrance = theme_picker_entrance(edge, metrics::PICKER_WIDTH);
        let content: super::super::native_panel::PanelContent =
            Rc::new(move |transition, _, cx| {
                let Some(space) = Settings::get(cx).spaces.get(space).cloned() else {
                    return div().into_any_element();
                };
                let picker = theme_picker::theme_picker(
                    ("window-theme-picker", space.id),
                    space.color,
                    theme::space_colors(&space),
                    Some(space.id),
                    theme_picker::change_space_theme(space.id),
                )
                .into_floating()
                .over_glass(true);
                div()
                    .id("theme-picker-dialog")
                    .role(Role::Dialog)
                    .aria_label(format!("Edit theme for {}", space.name))
                    .size_full()
                    .child(picker)
                    .animate_with_offset_stable_id(
                        "window-theme-picker-motion",
                        entrance,
                        transition,
                        &Settings::motion(cx),
                        (0.0, 0.0),
                    )
            });
        let browser = cx.entity().downgrade();
        let browser_window = window.window_handle();
        let on_dismiss = Rc::new(move |cx: &mut gpui::App| {
            let browser = browser.clone();
            let _ = browser_window.update(cx, |_, window, cx| {
                let _ = browser.update(cx, |this, cx| this.dismiss_theme_picker(window, cx));
            });
        });
        self.theme_picker_panel.open(
            window,
            super::super::native_panel::PanelOptions {
                bounds: gpui::Bounds::new(
                    gpui::point(px(left), px(top)),
                    gpui::size(px(metrics::PICKER_WIDTH), px(metrics::PICKER_POPUP_HEIGHT)),
                ),
                corner_radius: metrics::theme_accent_panel_radius(),
                entrance,
                content,
                on_dismiss,
            },
            cx,
        );
    }

    pub(super) fn dismiss_theme_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.theme_picker_space.take().is_some() {
            #[cfg(target_os = "macos")]
            self.theme_picker_panel.close(cx);
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
        // The native panel draws the picker in its own window.
        #[cfg(target_os = "macos")]
        if self.theme_picker_panel.is_open() {
            return None;
        }
        let requested = self
            .theme_picker_space
            .filter(|id| Settings::get(cx).spaces.get(*id).is_some());
        if requested.is_none() {
            self.theme_picker_space = None;
        }
        let animations = Settings::motion(cx);
        let (left, top, edge) = self.picker_popup_position(window, cx);
        let picker_motion = theme_picker_entrance(edge, metrics::PICKER_WIDTH);
        let (space_id, transition) =
            self.theme_picker_presence
                .sync(requested, picker_motion, &animations, cx)?;
        let space = Settings::get(cx).spaces.get(space_id)?.clone();
        let dialog_label = format!("Edit theme for {}", space.name);
        let over_glass = glass_available(cx);
        let picker = theme_picker::theme_picker(
            ("window-theme-picker", space_id),
            space.color,
            theme::space_colors(&space),
            Some(space_id),
            theme_picker::change_space_theme(space_id),
        )
        .into_floating()
        .over_glass(over_glass);
        // The picker paints its own surface where there is no glass.
        let dialog = glass("window-theme-picker-surface")
            .z_index(layer::POPOVER)
            .rounded(px(metrics::theme_accent_panel_radius()))
            .motion(picker_motion, transition)
            .child(
                div()
                    .id("theme-picker-dialog")
                    .role(Role::Dialog)
                    .aria_label(dialog_label)
                    .owns_clicks()
                    .child(picker),
            );
        let panel = dialog.absolute().left(px(left)).top(px(top));
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

    /// Where the picker opens, and the side it opens on, which it grows from.
    fn picker_popup_position(&self, window: &Window, cx: &Context<Self>) -> (f32, f32, Edge) {
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

        let (desired_left, desired_top, edge) = match (region, self.theme_picker_anchor) {
            (Some(DockRegion::Leading), _) => (
                sidebar_inset + sidebar_width + gap,
                centered_top,
                Edge::Left,
            ),
            (Some(DockRegion::Trailing), _) => (
                viewport_width - sidebar_inset - sidebar_width - width - gap,
                centered_top,
                Edge::Right,
            ),
            (_, Some(anchor)) => {
                let pointer_x = f32::from(anchor.x);
                let (left, edge) = if pointer_x + width + gap <= viewport_width {
                    (pointer_x + gap, Edge::Left)
                } else {
                    (pointer_x - width - gap, Edge::Right)
                };
                (left, f32::from(anchor.y) - height / 2.0, edge)
            }
            _ => (
                (viewport_width - width) / 2.0,
                (viewport_height - height) / 2.0,
                Edge::Bottom,
            ),
        };

        let max_left = (viewport_width - width - inset).max(inset);
        let max_top = (viewport_height - height - inset).max(inset);
        (
            desired_left.clamp(inset, max_left),
            desired_top.clamp(inset, max_top),
            edge,
        )
    }
}
