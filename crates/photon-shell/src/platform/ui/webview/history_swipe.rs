//! Native trackpad history-swipe capture for one tab.

use crate::platform::haptics::Feedback;
use gpui::{AnyElement, Context, ScrollWheelEvent, TouchPhase, div, prelude::*, px, rgba};
use photon_core::BrowserCommand;
use std::time::Instant;

use super::super::icons::{back_icon, forward_icon};
use super::super::input::physical_horizontal_delta;
use super::super::layout::Raised;
use super::super::{metrics, theme::ThemeColors};
use super::PhotonWebView;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum HistorySwipePhase {
    #[default]
    Idle,
    Tracking,
    Committing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HistoryDirection {
    Back,
    Forward,
}

impl HistoryDirection {
    fn progress_sign(self) -> f32 {
        match self {
            Self::Back => 1.0,
            Self::Forward => -1.0,
        }
    }

    fn command(self) -> BrowserCommand {
        match self {
            Self::Back => BrowserCommand::Back,
            Self::Forward => BrowserCommand::Forward,
        }
    }
}

#[derive(Default)]
pub(super) struct HistorySwipe {
    pub(super) phase: HistorySwipePhase,
    direction: Option<HistoryDirection>,
    gesture_id: u64,
    suppressed_gesture_id: u64,
    latest_gesture_id: u64,
    cancelled_gesture_id: u64,
    ended_gesture_id: u64,
    progress: f32,
    progress_velocity: f32,
    /// Whether the swipe has gone far enough to navigate when released,
    /// which a haptic tap marks as it crosses.
    armed: bool,
    last_input_at: Option<Instant>,
    last_physical_velocity: f32,
}

impl PhotonWebView {
    /// The arrow shown at the page edge while a history swipe is captured.
    pub(super) fn history_swipe_indicator(&self, palette: ThemeColors) -> Option<AnyElement> {
        let (back, x, opacity) = self.history_swipe_indicator_placement()?;
        let icon_size = metrics::HISTORY_SWIPE_INDICATOR_ICON_SIZE;
        let icon = if back {
            back_icon(palette.text_primary, icon_size).into_any_element()
        } else {
            forward_icon(palette.text_primary, icon_size).into_any_element()
        };
        let size = metrics::HISTORY_SWIPE_INDICATOR_SIZE;
        Some(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(px(x))
                .w(px(size))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .size(px(size))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .border_1()
                        .border_color(rgba(palette.menu_border))
                        .raised(palette)
                        .opacity(opacity)
                        .child(icon),
                )
                .into_any_element(),
        )
    }

    fn history_swipe_indicator_placement(&self) -> Option<(bool, f32, f32)> {
        if self.history_swipe.phase != HistorySwipePhase::Tracking {
            return None;
        }
        let direction = self.history_swipe.direction?;
        let width = self.page_width();
        if width <= 0.0 {
            return None;
        }
        let progress = self.history_swipe.progress;
        let size = metrics::HISTORY_SWIPE_INDICATOR_SIZE;
        let travel = (progress * 0.5 - size * 0.5).clamp(0.0, (width - size).max(0.0));
        let x = match direction {
            HistoryDirection::Back => travel,
            HistoryDirection::Forward => (width - size - travel).max(0.0),
        };
        let opacity = (progress / metrics::HISTORY_SWIPE_INDICATOR_REVEAL_DISTANCE).clamp(0.0, 1.0);
        Some((direction == HistoryDirection::Back, x, opacity))
    }

    /// Tracks native phases and captures an edge swipe for history navigation.
    pub(super) fn handle_history_swipe_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        gesture_id: u64,
        cx: &mut Context<Self>,
    ) -> bool {
        if event.touch_phase == TouchPhase::Started {
            let previous_gesture_id = self.history_swipe.latest_gesture_id;
            if previous_gesture_id != 0 && previous_gesture_id != gesture_id {
                let was_captured = self.history_swipe.suppressed_gesture_id == previous_gesture_id;
                self.reset_history_swipe(was_captured);
            }
            self.history_swipe.latest_gesture_id = gesture_id;
            self.history_swipe.cancelled_gesture_id = 0;
            self.history_swipe.ended_gesture_id = 0;
            self.history_swipe.last_input_at = Some(Instant::now());
            self.history_swipe.last_physical_velocity = 0.0;
        } else if gesture_id != 0 {
            self.history_swipe.latest_gesture_id = gesture_id;
        }

        let physical_delta = physical_horizontal_delta(event);
        let now = Instant::now();
        let sample_elapsed = (event.touch_phase != TouchPhase::Started)
            .then(|| {
                self.history_swipe
                    .last_input_at
                    .map(|previous| now.duration_since(previous).as_secs_f32().max(1.0 / 240.0))
            })
            .flatten();
        if !event.is_momentum && physical_delta.abs() > 0.0 {
            if let Some(elapsed) = sample_elapsed {
                self.history_swipe.last_physical_velocity = physical_delta / elapsed;
            }
            self.history_swipe.last_input_at = Some(now);
        }

        if self.history_swipe.phase == HistorySwipePhase::Tracking
            && self.history_swipe.gesture_id == gesture_id
            && !event.is_momentum
            && let Some(direction) = self.history_swipe.direction
        {
            let progress_delta = physical_delta * direction.progress_sign();
            if progress_delta != 0.0 {
                self.history_swipe.progress =
                    (self.history_swipe.progress + progress_delta).clamp(0.0, self.page_width());
                self.history_swipe.progress_velocity =
                    sample_elapsed.map_or(0.0, |elapsed| progress_delta / elapsed);
                self.follow_history_swipe_threshold(cx);
            }
        }

        if event.touch_phase == TouchPhase::Cancelled {
            self.history_swipe.cancelled_gesture_id = gesture_id;
            if self.history_swipe.phase == HistorySwipePhase::Tracking
                && self.history_swipe.gesture_id == gesture_id
            {
                self.finish_history_swipe();
            }
        } else if event.touch_phase == TouchPhase::Ended {
            self.history_swipe.ended_gesture_id = gesture_id;
            if self.history_swipe.phase == HistorySwipePhase::Tracking
                && self.history_swipe.gesture_id == gesture_id
            {
                self.settle_history_swipe(cx);
            }
        }

        let captured = self.history_swipe.suppressed_gesture_id == gesture_id && gesture_id != 0;
        if event.is_momentum_ended {
            self.history_swipe.suppressed_gesture_id = 0;
        }
        if event.touch_phase == TouchPhase::Cancelled {
            self.history_swipe.suppressed_gesture_id = 0;
            self.history_swipe.latest_gesture_id = 0;
        }
        captured
    }

    /// Called only after Engine confirms horizontal movement escaped the root
    /// scroll chain with CSS overscroll behavior allowing it.
    pub(in crate::platform) fn history_swipe_candidate(
        &mut self,
        gesture_id: u64,
        back: bool,
        unconsumed_delta: f32,
        cx: &mut Context<Self>,
    ) {
        let matches_gesture = is_current_or_ended_gesture(
            gesture_id,
            self.history_swipe.latest_gesture_id,
            self.history_swipe.ended_gesture_id,
        );
        super::super::super::trace(format_args!(
            "history-swipe-candidate id={gesture_id} latest={} ended={} cancelled={} back={back} delta={unconsumed_delta:.2} history=({}, {}) phase={:?}",
            self.history_swipe.latest_gesture_id,
            self.history_swipe.ended_gesture_id,
            self.history_swipe.cancelled_gesture_id,
            self.state.can_go_back,
            self.state.can_go_forward,
            self.history_swipe.phase,
        ));
        if gesture_id == 0
            || !matches_gesture
            || gesture_id == self.history_swipe.cancelled_gesture_id
            || gesture_id == self.history_swipe.suppressed_gesture_id
            || (back && !self.state.can_go_back)
            || (!back && !self.state.can_go_forward)
        {
            return;
        }

        // Compositor callbacks are event-ordered. Only the first edge result
        // starts capture; later raw samples track directly.
        if self.history_swipe.phase == HistorySwipePhase::Tracking
            || self.history_swipe.phase == HistorySwipePhase::Committing
        {
            return;
        }

        let direction = if back {
            HistoryDirection::Back
        } else {
            HistoryDirection::Forward
        };
        let width = self.page_width();
        let device_scale = self
            .last_viewport
            .map_or(1.0, |viewport| f32::from_bits(viewport.2));
        let unconsumed_delta = unconsumed_delta / (device_scale * self.zoom_level as f32).max(0.01);
        if width <= 0.0 || unconsumed_delta <= 0.0 {
            return;
        }

        self.history_swipe.phase = HistorySwipePhase::Tracking;
        self.history_swipe.direction = Some(direction);
        self.history_swipe.gesture_id = gesture_id;
        self.history_swipe.suppressed_gesture_id = gesture_id;
        self.history_swipe.progress = unconsumed_delta.min(width);
        self.history_swipe.progress_velocity =
            self.history_swipe.last_physical_velocity * direction.progress_sign();

        if self.history_swipe.ended_gesture_id == gesture_id {
            self.settle_history_swipe(cx);
        }
    }

    pub(in crate::platform) fn history_swipe_navigation_committed(&mut self) {
        if self.history_swipe.phase == HistorySwipePhase::Committing {
            self.finish_history_swipe();
        }
    }

    pub(in crate::platform) fn history_swipe_navigation_failed(&mut self) {
        if self.history_swipe.phase == HistorySwipePhase::Tracking
            || self.history_swipe.phase == HistorySwipePhase::Committing
        {
            self.finish_history_swipe();
        }
    }

    /// A tab switch, focus loss, or unrelated navigation ends capture cleanly.
    pub(in crate::platform) fn reset_history_swipe(&mut self, end_engine_gesture: bool) {
        let gesture_id = self.history_swipe.suppressed_gesture_id;
        self.history_swipe = HistorySwipe::default();
        if end_engine_gesture {
            self.input.end_history_swipe(&mut self.session, gesture_id);
        }
    }

    fn settle_history_swipe(&mut self, cx: &mut Context<Self>) {
        let Some(direction) = self.history_swipe.direction else {
            return;
        };
        let width = self.page_width();
        let can_navigate = match direction {
            HistoryDirection::Back => self.state.can_go_back,
            HistoryDirection::Forward => self.state.can_go_forward,
        };
        if !should_commit_history_swipe(
            self.history_swipe.progress,
            self.history_swipe.progress_velocity,
            width,
            can_navigate,
        ) {
            self.finish_history_swipe();
            return;
        }

        self.history_swipe.phase = HistorySwipePhase::Committing;
        let reply = self.dialogs.navigation_started();
        self.send_dialog_reply(reply, cx);
        if self
            .crash_notice
            .as_ref()
            .is_some_and(|notice| !matches!(notice, super::crashes::CrashNotice::Reloading))
        {
            self.crash_notice = None;
            self.state_changed(cx);
        }
        if let Err(error) = self.session.execute(direction.command()) {
            trace_swipe_error(&error);
            self.finish_history_swipe();
        }
    }

    fn finish_history_swipe(&mut self) {
        self.history_swipe.phase = HistorySwipePhase::Idle;
        self.history_swipe.direction = None;
        self.history_swipe.gesture_id = 0;
        self.history_swipe.progress = 0.0;
        self.history_swipe.progress_velocity = 0.0;
        self.history_swipe.armed = false;
    }

    /// Taps the trackpad as the swipe passes the distance at which
    /// releasing it navigates; drawing back below it disarms it quietly.
    fn follow_history_swipe_threshold(&mut self, cx: &mut Context<Self>) {
        let armed = self.history_swipe.progress
            >= self.page_width() * metrics::HISTORY_SWIPE_COMMIT_FRACTION;
        if armed != self.history_swipe.armed {
            self.history_swipe.armed = armed;
            if armed {
                crate::platform::haptics::play(cx, Feedback::Alignment);
            }
        }
    }

    fn page_width(&self) -> f32 {
        self.last_viewport.map_or(0.0, |viewport| viewport.0 as f32)
    }
}

fn should_commit_history_swipe(
    progress: f32,
    outward_velocity: f32,
    width: f32,
    can_navigate: bool,
) -> bool {
    can_navigate
        && width > 0.0
        && (progress >= width * metrics::HISTORY_SWIPE_COMMIT_FRACTION
            || (progress >= width * metrics::HISTORY_SWIPE_FLICK_FRACTION
                && outward_velocity >= metrics::HISTORY_SWIPE_FLICK_VELOCITY))
}

fn is_current_or_ended_gesture(gesture_id: u64, current: u64, ended: u64) -> bool {
    gesture_id != 0 && (gesture_id == current || gesture_id == ended)
}

fn trace_swipe_error(error: &anyhow::Error) {
    super::super::super::trace(format_args!("history swipe navigation failed: {error:#}"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slow_swipe_commits_after_crossing_distance_threshold() {
        let width = 1000.0;
        assert!(should_commit_history_swipe(
            width * metrics::HISTORY_SWIPE_COMMIT_FRACTION,
            0.0,
            width,
            true,
        ));
    }

    #[test]
    fn incomplete_swipe_cancels_without_a_fast_flick() {
        let width = 1000.0;
        assert!(!should_commit_history_swipe(100.0, 200.0, width, true));
    }

    #[test]
    fn fast_outward_flick_commits_at_lower_progress() {
        let width = 1000.0;
        assert!(should_commit_history_swipe(
            width * metrics::HISTORY_SWIPE_FLICK_FRACTION,
            metrics::HISTORY_SWIPE_FLICK_VELOCITY,
            width,
            true,
        ));
    }

    #[test]
    fn reversing_before_threshold_cancels_even_when_the_swipe_was_fast() {
        let width = 1000.0;
        assert!(!should_commit_history_swipe(200.0, -1200.0, width, true));
    }

    #[test]
    fn swipe_never_commits_without_matching_history() {
        assert!(!should_commit_history_swipe(500.0, 1500.0, 1000.0, false));
    }

    #[test]
    fn accepts_edge_result_arriving_after_native_gesture_end() {
        assert!(is_current_or_ended_gesture(7, 0, 7));
        assert!(!is_current_or_ended_gesture(6, 0, 7));
        assert!(!is_current_or_ended_gesture(0, 0, 0));
    }
}
