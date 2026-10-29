//! The sidebar beside the page: with its labels or collapsed to icons, or
//! hidden, and while hidden, revealed over the page by pointing at the
//! window's outer edge. Zen mode hides it with the rest of the chrome. Each
//! moves smoothly, and the page follows.
//!
//! - `commands`: showing, hiding, collapsing and Zen mode, and the macOS
//!   window controls that follow the sidebar.
//! - `edges`: resizing by dragging its edge, and revealing it from the
//!   window's edge.
//! - `view`: drawing it.

mod commands;
mod edges;
mod view;

use gpui::{App, Task};
use photon_core::DockRegion;
use std::time::Instant;

use super::super::metrics;
use super::super::motion::{
    MotionConfig, SIDEBAR_LAYOUT_SPRING, SIDEBAR_RESIZE_SPRING, SIDEBAR_REVEAL_SPRING, Speed,
    Tween, mix,
};
use super::super::settings::Settings;
use super::super::workspace_layout::WorkspaceLayout;

/// Where the macOS native window controls sit, which follows the sidebar.
#[derive(Clone, Copy, PartialEq)]
struct WindowControls {
    visible: bool,
    revealed: bool,
    /// Stacked at the top of the collapsed sidebar, rather than in a row.
    column: bool,
}

pub(super) struct SidebarState {
    /// Whether it is kept beside the page rather than hidden.
    visible: bool,
    /// Whether it shows only icons while beside the page.
    collapsed: bool,
    /// Whether Zen mode hides it, and all other chrome, around the page.
    zen: bool,
    /// How far it is shown beside the page, from 0 to 1.
    shown: Tween,
    /// How far it is expanded from icons to labels, from 0 to 1.
    expanded: Tween,
    /// How far Zen mode has tucked the chrome away, from 0 to 1.
    tucked: Tween,
    /// Whether it is revealed over the page while hidden.
    revealed: bool,
    /// How far it is revealed, from 0 to 1.
    reveal: Tween,
    /// Whether the pointer is over the reveal edge, and over the sidebar.
    pointer_on_edge: bool,
    pointer_on_sidebar: bool,
    _hide_check: Option<Task<()>>,
    /// The width while its edge is dragged, until it is saved.
    dragged_width: Option<f32>,
    /// Temporary width beyond the maximum, which springs back after a drag.
    resize_overshoot: Tween,
    /// Whether the resize edge is currently being dragged.
    resize_dragging: bool,
    _save_width: Option<Task<()>>,
    /// The target window control layout, so it changes only when needed.
    window_controls: Option<WindowControls>,
    /// The moment this frame shows. Every part of the window that follows
    /// the sidebar samples its motion here, so the sidebar, the page and the
    /// window controls move in step rather than a few microseconds apart.
    frame: Instant,
    /// Whether motion is sampled at its targets, as in `at_rest`.
    at_rest: bool,
    /// The window it is in, whose space chooses its layout.
    window_id: u64,
}

impl SidebarState {
    pub(super) fn new(window_id: u64, cx: &App) -> Self {
        let collapsed = Settings::get(cx).sidebar.collapsed;
        Self {
            visible: true,
            collapsed,
            zen: false,
            shown: Tween::at_with_curve(1.0, SIDEBAR_LAYOUT_SPRING),
            expanded: Tween::at_with_curve(progress(!collapsed), SIDEBAR_LAYOUT_SPRING),
            tucked: Tween::at_with_curve(0.0, SIDEBAR_LAYOUT_SPRING),
            revealed: false,
            reveal: Tween::at_with_curve(0.0, SIDEBAR_REVEAL_SPRING),
            pointer_on_edge: false,
            pointer_on_sidebar: false,
            _hide_check: None,
            dragged_width: None,
            resize_overshoot: Tween::at_with_curve(0.0, SIDEBAR_RESIZE_SPRING),
            resize_dragging: false,
            _save_width: None,
            window_controls: None,
            frame: Instant::now(),
            at_rest: false,
            window_id,
        }
    }

    /// The workspace layout in its window's space.
    fn layout(&self, cx: &App) -> std::rc::Rc<WorkspaceLayout> {
        let space = super::super::window_space::space_of(self.window_id, cx).id;
        WorkspaceLayout::current(space, cx)
    }

    /// Starts a frame: its motion is sampled at this moment until the next.
    pub(super) fn begin_frame(&mut self) {
        self.frame = Instant::now();
    }

    /// How wide it is with its labels: as dragged, or as saved.
    pub(super) fn width(&self, cx: &App) -> f32 {
        self.dragged_width.unwrap_or_else(|| {
            self.layout(cx)
                .sidebar_extent()
                .unwrap_or_else(|| Settings::get(cx).sidebar.width()) as f32
        }) + self.sample(&self.resize_overshoot)
    }

    /// How wide it is beside the page, between icons and labels.
    pub(super) fn docked_width(&self, cx: &App) -> f32 {
        mix(
            metrics::SIDEBAR_COLLAPSED_WIDTH,
            self.width(cx),
            self.sample(&self.expanded),
        )
    }

    /// Whether it is kept beside the page: shown, and not hidden by Zen mode.
    pub(super) fn docked(&self) -> bool {
        self.visible && !self.zen
    }

    /// Whether it shows only icons beside the page.
    pub(super) fn collapsed(&self) -> bool {
        self.collapsed
    }

    pub(super) fn zen(&self) -> bool {
        self.zen
    }

    /// How far it is shown beside the page, from 0 to 1.
    pub(super) fn shown(&self) -> f32 {
        self.sample(&self.shown)
    }

    /// The live panel offset, shared by the sidebar and its native controls.
    pub(super) fn panel_offset(&self, cx: &App) -> f32 {
        let out = self.shown().max(self.revealed());
        let width = if self.over_page() {
            self.width(cx)
        } else {
            self.docked_width(cx)
        };
        (out - 1.0) * width
    }

    /// How far the bars around the page are shown, from 0 to 1: Zen mode
    /// tucks them away.
    pub(super) fn chrome_shown(&self) -> f32 {
        1.0 - self.sample(&self.tucked)
    }

    /// How far it is revealed over the page, from 0 to 1.
    fn revealed(&self) -> f32 {
        self.sample(&self.reveal)
    }

    /// Whether it sits over the page, revealed or on its way to being kept.
    pub(super) fn over_page(&self) -> bool {
        self.revealed || self.revealed() > 0.0
    }

    /// Whether it shows only icons now: collapsed beside the page. Revealed
    /// over the page, it always shows its labels.
    pub(super) fn shows_icons(&self) -> bool {
        self.collapsed && !self.over_page()
    }

    pub(super) fn is_moving(&self) -> bool {
        self.resize_dragging || self.is_tweening()
    }

    /// Whether it is moving on its own, rather than following a dragged edge.
    pub(super) fn is_tweening(&self) -> bool {
        [
            &self.shown,
            &self.expanded,
            &self.tucked,
            &self.reveal,
            &self.resize_overshoot,
        ]
        .into_iter()
        .any(|tween| tween.is_running_at(self.frame))
    }

    /// Runs `sample` with every tween at rest, as it will be once its motion
    /// ends, then returns to this frame.
    pub(super) fn at_rest<R>(&mut self, sample: impl FnOnce(&Self) -> R) -> R {
        self.at_rest = true;
        let rest = sample(self);
        self.at_rest = false;
        rest
    }

    /// A tween's value this frame, or its target while sampling at rest.
    fn sample(&self, tween: &Tween) -> f32 {
        if self.at_rest {
            tween.target()
        } else {
            tween.value_at(self.frame)
        }
    }

    /// Finishes decorative tweens if the application or platform disables motion.
    pub(super) fn settle_if_motion_disabled(&mut self, config: &MotionConfig) {
        if config.allows_motion() {
            return;
        }
        for tween in [
            &mut self.shown,
            &mut self.expanded,
            &mut self.tucked,
            &mut self.reveal,
            &mut self.resize_overshoot,
        ] {
            tween.jump_to(tween.target());
        }
    }

    /// Once a revealed sidebar has been kept shown and the page has made
    /// room for it, it is no longer revealed.
    pub(super) fn settle(&mut self) {
        if self.docked() && self.revealed && !self.shown.is_running_at(self.frame) {
            self.revealed = false;
            self.reveal.jump_to(0.0);
        }
    }

    /// Moves beside the page or away from it, as `visible` and `zen` ask.
    /// Hidden, it leaves with the page.
    fn follow_docked(&mut self, cx: &App) {
        let docked = self.docked();
        self.shown
            .animate_to(progress(docked), Speed::Standard, &Settings::motion(cx));
        if !docked {
            self.revealed = false;
            self.reveal.jump_to(0.0);
        }
    }

    fn set_visible(&mut self, visible: bool, cx: &App) -> bool {
        if self.visible == visible && (visible || self.zen) {
            return false;
        }
        self.visible = visible;
        // Fully hiding the sidebar hides the rest of the browser chrome too.
        // Showing it again leaves Zen mode, while an already visible sidebar
        // can still be hidden by Zen mode independently.
        self.zen = !visible;
        self.tucked
            .animate_to(progress(self.zen), Speed::Gentle, &Settings::motion(cx));
        self.follow_docked(cx);
        true
    }

    /// Keeps it beside the page, out of Zen mode. Clicking the toggle on a
    /// revealed sidebar pins it in place without animating the page reflow.
    fn dock(&mut self, cx: &App) {
        if self.docked() {
            return;
        }
        let was_revealed = self.over_page();
        self.visible = true;
        self.zen = false;
        if was_revealed {
            self.shown.jump_to(1.0);
            self.tucked.jump_to(0.0);
        } else {
            self.tucked
                .animate_to(0.0, Speed::Gentle, &Settings::motion(cx));
            self.follow_docked(cx);
        }
    }

    fn set_zen(&mut self, zen: bool, cx: &App) -> bool {
        if self.zen == zen {
            return false;
        }
        self.zen = zen;
        self.tucked
            .animate_to(progress(zen), Speed::Gentle, &Settings::motion(cx));
        self.follow_docked(cx);
        true
    }

    /// Collapses it to icons or expands it to labels. A revealed sidebar is
    /// already drawn with its labels, so kept from a reveal it expands at
    /// once.
    fn set_collapsed(&mut self, collapsed: bool, cx: &App) -> bool {
        if self.collapsed == collapsed {
            return false;
        }
        self.collapsed = collapsed;
        let target = progress(!collapsed);
        if self.over_page() {
            self.expanded.jump_to(target);
        } else {
            self.expanded
                .animate_to(target, Speed::Standard, &Settings::motion(cx));
        }
        true
    }

    fn set_revealed(&mut self, revealed: bool, cx: &App) -> bool {
        if self.revealed == revealed || (revealed && self.docked()) {
            return false;
        }
        self.revealed = revealed;
        self.reveal
            .animate_to(progress(revealed), Speed::Quick, &Settings::motion(cx));
        true
    }

    /// Where the native window controls belong: hidden with the sidebar and
    /// in Zen mode, back while it is revealed, and stacked when it is
    /// collapsed beside the page at the window's leading edge.
    fn window_controls(&self, region: Option<DockRegion>) -> WindowControls {
        let visible = if self.zen {
            self.revealed
        } else {
            region.is_none() || self.visible || self.revealed
        };
        WindowControls {
            visible,
            revealed: self.over_page(),
            column: region == Some(DockRegion::Leading)
                && self.docked()
                && self.collapsed
                && !self.revealed,
        }
    }
}

/// A tween's resting value for something on or off.
fn progress(on: bool) -> f32 {
    if on { 1.0 } else { 0.0 }
}
