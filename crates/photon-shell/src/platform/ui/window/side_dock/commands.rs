//! Showing, hiding and collapsing the sidebar, Zen mode, and the native
//! window controls that follow them.

use gpui::{App, Context, Window};

use super::super::super::settings::Settings;
use super::super::super::workspace_layout::{OmniboxPlacement, WorkspaceLayout};
use super::super::BrowserWindow;

impl BrowserWindow {
    /// Shows or hides the sidebar, and leaves Zen mode for it. A revealed
    /// sidebar is kept open with its labels. The horizontal layout has none.
    pub(in super::super) fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        if !WorkspaceLayout::current(cx).has_sidebar() {
            return;
        }
        if self.sidebar.over_page() {
            self.keep_sidebar(false, cx);
        } else if self.sidebar.docked() {
            self.set_sidebar_visible(false, cx);
        } else {
            self.keep_sidebar(self.sidebar.collapsed, cx);
        }
    }

    /// The sidebar's own button: expands it from icons, and otherwise
    /// toggles it.
    pub(in super::super) fn press_sidebar_toggle(&mut self, cx: &mut Context<Self>) {
        if self.sidebar.docked() && self.sidebar.shows_icons() {
            self.set_sidebar_collapsed(false, cx);
        } else {
            self.toggle_sidebar(cx);
        }
    }

    /// Collapses the sidebar to icons, or expands it again. A hidden
    /// sidebar comes back as icons.
    pub(in super::super) fn toggle_sidebar_icons(&mut self, cx: &mut Context<Self>) {
        if !WorkspaceLayout::current(cx).has_sidebar() {
            return;
        }
        let collapsed = !(self.sidebar.docked() && self.sidebar.collapsed);
        self.keep_sidebar(collapsed, cx);
    }

    /// Hides all chrome around the page, or brings it back as it was.
    pub(in super::super) fn toggle_zen_mode(&mut self, cx: &mut Context<Self>) {
        if self.sidebar.set_zen(!self.sidebar.zen, cx) {
            cx.notify();
        }
    }

    /// Shows the address field: leaves Zen mode, and expands the sidebar
    /// when the field lives there.
    pub(in super::super) fn show_address_field(&mut self, cx: &mut Context<Self>) {
        let layout = WorkspaceLayout::current(cx);
        if layout.has_sidebar() && layout.omnibox_placement() == OmniboxPlacement::Sidebar {
            self.keep_sidebar(false, cx);
        } else if self.sidebar.set_zen(false, cx) {
            cx.notify();
        }
    }

    /// Keeps the sidebar beside the page, collapsed or not, out of Zen mode.
    fn keep_sidebar(&mut self, collapsed: bool, cx: &mut Context<Self>) {
        self.sidebar.dock(cx);
        self.set_sidebar_collapsed(collapsed, cx);
        cx.notify();
    }

    pub(super) fn set_sidebar_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        if self.sidebar.set_visible(visible, cx) {
            cx.notify();
        }
    }

    /// Collapses or expands the sidebar, and remembers it for new windows.
    pub(super) fn set_sidebar_collapsed(&mut self, collapsed: bool, cx: &mut Context<Self>) {
        if self.sidebar.set_collapsed(collapsed, cx) {
            Settings::update(cx, |settings| settings.sidebar.collapsed = collapsed);
            cx.notify();
        }
    }

    /// Moves the native window controls to follow the sidebar.
    pub(in super::super) fn place_window_controls(&mut self, window: &Window, cx: &App) {
        let animations = Settings::get(cx).animations;
        window.set_traffic_light_fade_durations(
            animations.traffic_light_fade_in_ms(),
            animations.traffic_light_fade_out_ms(),
        );
        let region = WorkspaceLayout::current(cx).sidebar_region();
        let controls = self.sidebar.window_controls(region);
        #[cfg(target_os = "macos")]
        {
            let offset = if region == Some(photon_core::DockRegion::Leading) {
                self.sidebar.panel_offset(cx)
            } else {
                0.0
            };
            window.set_traffic_light_offset(gpui::point(gpui::px(offset), gpui::px(0.0)));
        }
        if self.sidebar.window_controls == Some(controls) {
            return;
        }
        self.sidebar.window_controls = Some(controls);
        #[cfg(target_os = "macos")]
        {
            if controls.visible {
                use super::super::super::metrics;
                let (axis, (x, y)) = if controls.column {
                    (gpui::Axis::Vertical, metrics::WINDOW_CONTROLS_COLUMN_ORIGIN)
                } else {
                    (gpui::Axis::Horizontal, metrics::WINDOW_CONTROLS_ORIGIN)
                };
                window.set_traffic_light_axis(axis);
                window.set_traffic_light_position(gpui::point(gpui::px(x), gpui::px(y)));
                window.set_traffic_lights_visible(true);
            } else {
                // Keep their current layout while the shared panel offset carries
                // them out with the sidebar and they fade away.
                window.set_traffic_lights_visible(false);
            }
        }
        #[cfg(not(target_os = "macos"))]
        let _ = window;
    }
}
