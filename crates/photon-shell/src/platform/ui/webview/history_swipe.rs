//! Native trackpad history-swipe capture and settling for one tab.

use gpui::{
    AnyElement, Context, ScrollDelta, ScrollWheelEvent, SpringConfig, SpringState, TouchPhase, div,
    prelude::*, px, rgba,
};
use photon_core::BrowserCommand;
use std::time::{Duration, Instant};

use super::super::icons::{back_icon, forward_icon};
use super::super::layout::Raised;
use super::super::{metrics, theme::ThemeColors};
use super::PhotonWebView;

const ANIMATION_INTERVAL: Duration = Duration::from_millis(8);
const SPRING_EPSILON: f32 = 0.35;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum HistorySwipePhase {
    #[default]
    Idle,
    Tracking,
    Committing,
    Cancelling,
    Completed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HistoryDirection {
    Back,
    Forward,
}

impl HistoryDirection {
    fn offset_sign(self) -> f32 {
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
    pub(super) offset: f32,
    progress_velocity: f32,
    last_input_at: Option<Instant>,
    last_physical_velocity: f32,
    animation_generation: u64,
    animation_target: Option<f32>,
    navigation_committed: bool,
    surface_replaced_during_commit: bool,
    animating_incoming_surface: bool,
}

impl PhotonWebView {
    /// The arrow shown in the page edge a swipe uncovers, pointing the way
    /// the tab will go.
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

    /// Whether the arrow points back, its left edge, and its opacity.
    fn history_swipe_indicator_placement(&self) -> Option<(bool, f32, f32)> {
        if !matches!(
            self.history_swipe.phase,
            HistorySwipePhase::Tracking
                | HistorySwipePhase::Cancelling
                | HistorySwipePhase::Committing
        ) || self.history_swipe.animating_incoming_surface
        {
            return None;
        }
        let direction = self.history_swipe.direction?;
        let width = self.page_width();
        if width <= 0.0 {
            return None;
        }
        let progress = self.history_swipe.offset.abs();
        let size = metrics::HISTORY_SWIPE_INDICATOR_SIZE;
        let travel = (progress * 0.5 - size * 0.5).clamp(0.0, (width - size).max(0.0));
        let x = match direction {
            HistoryDirection::Back => travel,
            HistoryDirection::Forward => (width - size - travel).max(0.0),
        };
        let opacity = (progress / metrics::HISTORY_SWIPE_INDICATOR_REVEAL_DISTANCE).clamp(0.0, 1.0);
        Some((direction == HistoryDirection::Back, x, opacity))
    }

    /// Tracks raw native phases and returns true once the compositor has
    /// transferred this gesture from page scrolling to history navigation.
    pub(super) fn handle_history_swipe_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        gesture_id: u64,
        cx: &mut Context<Self>,
    ) -> bool {
        if event.touch_phase == TouchPhase::Started {
            let previous_gesture_id = self.history_swipe.latest_gesture_id;
            if previous_gesture_id != 0 && previous_gesture_id != gesture_id {
                let was_captured = matches!(
                    self.history_swipe.phase,
                    HistorySwipePhase::Tracking | HistorySwipePhase::Cancelling
                );
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
        {
            if let Some(direction) = self.history_swipe.direction {
                let progress_delta = physical_delta * direction.offset_sign();
                if progress_delta != 0.0 {
                    let progress = self.history_swipe.offset * direction.offset_sign();
                    let progress = (progress + progress_delta).clamp(0.0, self.page_width());
                    self.history_swipe.offset = direction.offset_sign() * progress;
                    self.history_swipe.progress_velocity =
                        sample_elapsed.map_or(0.0, |elapsed| progress_delta / elapsed);
                    cx.notify();
                }
            }
        }

        if event.touch_phase == TouchPhase::Cancelled {
            self.history_swipe.cancelled_gesture_id = gesture_id;
            if self.history_swipe.phase == HistorySwipePhase::Tracking
                && self.history_swipe.gesture_id == gesture_id
            {
                self.cancel_history_swipe_animation(cx);
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
        if matches!(
            self.history_swipe.phase,
            HistorySwipePhase::Tracking | HistorySwipePhase::Committing
        ) {
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

        self.stop_history_swipe_animation();
        self.history_swipe.phase = HistorySwipePhase::Tracking;
        self.history_swipe.direction = Some(direction);
        self.history_swipe.gesture_id = gesture_id;
        self.history_swipe.suppressed_gesture_id = gesture_id;
        self.history_swipe.offset = direction.offset_sign() * unconsumed_delta.min(width);
        self.history_swipe.progress_velocity =
            self.history_swipe.last_physical_velocity * direction.offset_sign();
        self.history_swipe.navigation_committed = false;
        self.history_swipe.surface_replaced_during_commit = false;
        self.history_swipe.animating_incoming_surface = false;
        cx.notify();

        if self.history_swipe.ended_gesture_id == gesture_id {
            self.settle_history_swipe(cx);
        }
    }

    pub(in crate::platform) fn history_swipe_navigation_committed(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if self.history_swipe.phase != HistorySwipePhase::Committing {
            return;
        }
        self.history_swipe.navigation_committed = true;
        if self.history_swipe.surface_replaced_during_commit {
            self.animate_incoming_history_surface(cx);
        }
    }

    pub(in crate::platform) fn history_swipe_navigation_failed(&mut self, cx: &mut Context<Self>) {
        if matches!(
            self.history_swipe.phase,
            HistorySwipePhase::Tracking | HistorySwipePhase::Committing
        ) {
            self.history_swipe.phase = HistorySwipePhase::Cancelling;
            self.history_swipe.animating_incoming_surface = false;
            self.animate_history_swipe_to(0.0, cx);
        }
    }

    pub(super) fn history_swipe_surface_replaced(&mut self, cx: &mut Context<Self>) {
        if self.history_swipe.phase == HistorySwipePhase::Committing
            && self.history_swipe.navigation_committed
        {
            self.history_swipe.surface_replaced_during_commit = true;
            self.animate_incoming_history_surface(cx);
        }
    }

    /// A tab switch, focus loss, or unrelated navigation must never carry an
    /// in-progress displacement into the next visible page.
    pub(in crate::platform) fn reset_history_swipe(&mut self, end_engine_gesture: bool) {
        let gesture_id = self.history_swipe.suppressed_gesture_id;
        self.stop_history_swipe_animation();
        self.history_swipe.phase = HistorySwipePhase::Idle;
        self.history_swipe.direction = None;
        self.history_swipe.gesture_id = 0;
        self.history_swipe.suppressed_gesture_id = 0;
        self.history_swipe.latest_gesture_id = 0;
        self.history_swipe.offset = 0.0;
        self.history_swipe.progress_velocity = 0.0;
        self.history_swipe.animation_target = None;
        self.history_swipe.navigation_committed = false;
        self.history_swipe.surface_replaced_during_commit = false;
        self.history_swipe.animating_incoming_surface = false;
        if end_engine_gesture {
            self.input.end_history_swipe(&mut self.session, gesture_id);
        }
    }

    fn cancel_history_swipe_animation(&mut self, cx: &mut Context<Self>) {
        if self.history_swipe.phase != HistorySwipePhase::Tracking {
            return;
        }
        self.history_swipe.phase = HistorySwipePhase::Cancelling;
        self.animate_history_swipe_to(0.0, cx);
    }

    fn settle_history_swipe(&mut self, cx: &mut Context<Self>) {
        let Some(direction) = self.history_swipe.direction else {
            return;
        };
        let width = self.page_width();
        let progress = self.history_swipe.offset.abs();
        let velocity = self.history_swipe.progress_velocity;
        let can_navigate = match direction {
            HistoryDirection::Back => self.state.can_go_back,
            HistoryDirection::Forward => self.state.can_go_forward,
        };
        let should_commit = should_commit_history_swipe(progress, velocity, width, can_navigate);

        if !should_commit {
            self.history_swipe.phase = HistorySwipePhase::Cancelling;
            self.animate_history_swipe_to(0.0, cx);
            return;
        }

        self.history_swipe.phase = HistorySwipePhase::Committing;
        self.history_swipe.navigation_committed = false;
        self.history_swipe.surface_replaced_during_commit = false;
        self.animate_history_swipe_to(direction.offset_sign() * width, cx);

        let reply = self.dialogs.navigation_started();
        self.send_dialog_reply(reply, cx);
        if let Some(notice) = self.crash_notice
            && notice != super::crashes::CrashNotice::Reloading
        {
            self.crash_notice = None;
            self.state_changed(cx);
        }
        if let Err(error) = self.session.execute(direction.command()) {
            trace_swipe_error(&error);
            self.history_swipe.phase = HistorySwipePhase::Cancelling;
            self.animate_history_swipe_to(0.0, cx);
        }
    }

    fn animate_incoming_history_surface(&mut self, cx: &mut Context<Self>) {
        let Some(direction) = self.history_swipe.direction else {
            self.reset_history_swipe(false);
            return;
        };
        self.history_swipe.animating_incoming_surface = true;
        self.history_swipe.offset = -direction.offset_sign() * self.page_width();
        self.animate_history_swipe_to(0.0, cx);
    }

    fn animate_history_swipe_to(&mut self, target: f32, cx: &mut Context<Self>) {
        self.stop_history_swipe_animation();
        self.history_swipe.animation_generation = self
            .history_swipe
            .animation_generation
            .wrapping_add(1)
            .max(1);
        let generation = self.history_swipe.animation_generation;
        let initial = SpringState {
            position: self.history_swipe.offset,
            velocity: self.history_swipe.progress_velocity
                * self
                    .history_swipe
                    .direction
                    .map_or(1.0, HistoryDirection::offset_sign),
        };
        self.history_swipe.animation_target = Some(target);

        if cx.reduce_motion() {
            self.finish_history_swipe_animation(target);
            cx.notify();
            return;
        }

        let spring = SpringConfig::new(
            metrics::HISTORY_SWIPE_SPRING_STIFFNESS,
            metrics::HISTORY_SWIPE_SPRING_DAMPING,
            1.0,
        );
        self.history_swipe_animation = Some(cx.spawn(async move |this, cx| {
            let mut spring_state = initial;
            let mut previous = Instant::now();
            loop {
                cx.background_executor().timer(ANIMATION_INTERVAL).await;
                let now = Instant::now();
                let elapsed = now.duration_since(previous).as_secs_f32().min(0.05);
                previous = now;
                spring_state = spring.step(spring_state, target, elapsed);
                let state = spring_state;
                let keep_running = this
                    .update(cx, |view, cx| {
                        if view.history_swipe.animation_generation != generation {
                            return false;
                        }
                        view.history_swipe.offset = state.position;
                        cx.notify();
                        if spring.is_settled(state, target, SPRING_EPSILON) {
                            view.finish_history_swipe_animation(target);
                            cx.notify();
                            false
                        } else {
                            true
                        }
                    })
                    .unwrap_or(false);
                if !keep_running {
                    break;
                }
            }
        }));
    }

    fn finish_history_swipe_animation(&mut self, target: f32) {
        self.history_swipe.offset = target;
        self.history_swipe.animation_target = None;
        let settled = match self.history_swipe.phase {
            HistorySwipePhase::Committing => self.history_swipe.animating_incoming_surface,
            HistorySwipePhase::Cancelling => true,
            _ => false,
        };
        if settled {
            self.history_swipe.animating_incoming_surface = false;
            self.history_swipe.phase = HistorySwipePhase::Completed;
            self.history_swipe.direction = None;
            self.history_swipe.gesture_id = 0;
            self.history_swipe.offset = 0.0;
        }
    }

    fn stop_history_swipe_animation(&mut self) {
        self.history_swipe_animation.take();
        self.history_swipe.animation_generation = self
            .history_swipe
            .animation_generation
            .wrapping_add(1)
            .max(1);
    }

    fn page_width(&self) -> f32 {
        self.last_viewport.map_or(0.0, |viewport| viewport.0 as f32)
    }
}

fn physical_horizontal_delta(event: &ScrollWheelEvent) -> f32 {
    let delta = match event.delta {
        ScrollDelta::Pixels(delta) => f32::from(delta.x),
        ScrollDelta::Lines(_) => 0.0,
    };
    physical_delta_from_device(delta, event.is_direction_inverted_from_device)
}

fn physical_delta_from_device(delta: f32, direction_inverted_from_device: bool) -> f32 {
    if direction_inverted_from_device {
        -delta
    } else {
        delta
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
    fn direction_inversion_keeps_navigation_tied_to_finger_motion() {
        assert_eq!(physical_delta_from_device(12.0, false), 12.0);
        assert_eq!(physical_delta_from_device(-12.0, true), 12.0);
        assert_eq!(physical_delta_from_device(-12.0, false), -12.0);
        assert_eq!(physical_delta_from_device(12.0, true), -12.0);
    }

    #[test]
    fn accepts_edge_result_arriving_after_native_gesture_end() {
        assert!(is_current_or_ended_gesture(7, 0, 7));
        assert!(!is_current_or_ended_gesture(6, 0, 7));
        assert!(!is_current_or_ended_gesture(0, 0, 0));
    }
}
