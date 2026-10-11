//! Mac trackpad overscroll at the page's root scroll boundary.

use gpui::{Context, ScrollDelta, ScrollWheelEvent, SpringConfig, SpringState, TouchPhase};
use std::time::{Duration, Instant};

use super::super::metrics;
use super::PhotonWebView;

const ANIMATION_INTERVAL: Duration = Duration::from_millis(8);
const SPRING_EPSILON: f32 = 0.35;
const MAX_INITIAL_VELOCITY: f32 = 2_500.0;

#[derive(Default)]
pub(super) struct VerticalOverscroll {
    pub(super) offset: f32,
    raw_offset: f32,
    latest_gesture_id: u64,
    ended_gesture_id: u64,
    cancelled_gesture_id: u64,
    last_candidate_at: Option<Instant>,
    last_velocity: f32,
    animation_generation: u64,
    settle_generation: u64,
}

impl PhotonWebView {
    /// Uses native-phase, precise wheel gestures only. Discrete wheel input and
    /// platforms without AppKit's phase data keep their existing behavior.
    pub(super) fn observe_vertical_overscroll_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        gesture_id: u64,
        cx: &mut Context<Self>,
    ) {
        if !is_precise_native_gesture(event) || gesture_id == 0 {
            return;
        }

        if event.touch_phase == TouchPhase::Started
            && self.vertical_overscroll.latest_gesture_id != gesture_id
        {
            self.stop_vertical_overscroll_animation();
            self.stop_vertical_overscroll_settle_timer();
            self.vertical_overscroll.raw_offset =
                raw_offset_from_visual(self.vertical_overscroll.offset, self.maximum_overscroll());
            self.vertical_overscroll.latest_gesture_id = gesture_id;
            self.vertical_overscroll.ended_gesture_id = 0;
            self.vertical_overscroll.cancelled_gesture_id = 0;
            self.vertical_overscroll.last_candidate_at = None;
            self.vertical_overscroll.last_velocity = 0.0;
        }

        if event.is_momentum {
            self.stop_vertical_overscroll_settle_timer();
        }

        if event.touch_phase == TouchPhase::Ended {
            self.vertical_overscroll.ended_gesture_id = gesture_id;
            self.schedule_vertical_overscroll_settle(gesture_id, false, cx);
        }

        if event.is_momentum_ended {
            self.vertical_overscroll.ended_gesture_id = gesture_id;
            self.schedule_vertical_overscroll_settle(gesture_id, true, cx);
        }

        if event.touch_phase == TouchPhase::Cancelled {
            self.vertical_overscroll.cancelled_gesture_id = gesture_id;
            self.vertical_overscroll.ended_gesture_id = gesture_id;
            self.schedule_vertical_overscroll_settle(gesture_id, true, cx);
        }
    }

    /// Called only when Engine reports unconsumed vertical movement at the
    /// root viewport and the page's CSS overscroll behavior allows an effect.
    pub(in crate::platform) fn vertical_overscroll_candidate(
        &mut self,
        gesture_id: u64,
        unconsumed_delta: f32,
        cx: &mut Context<Self>,
    ) {
        if gesture_id == 0
            || (gesture_id != self.vertical_overscroll.latest_gesture_id
                && gesture_id != self.vertical_overscroll.ended_gesture_id)
            || gesture_id == self.vertical_overscroll.cancelled_gesture_id
            || cx.reduce_motion()
        {
            return;
        }

        self.stop_vertical_overscroll_settle_timer();
        self.stop_vertical_overscroll_animation();

        let scale = self.device_scale();
        let visual_delta = -unconsumed_delta / scale;
        if visual_delta == 0.0 {
            return;
        }

        let now = Instant::now();
        let previous_offset = self.vertical_overscroll.offset;
        self.vertical_overscroll.raw_offset += visual_delta;
        self.vertical_overscroll.offset = rubberband(
            self.vertical_overscroll.raw_offset,
            self.maximum_overscroll(),
        );
        if let Some(previous) = self.vertical_overscroll.last_candidate_at {
            let elapsed = now.duration_since(previous).as_secs_f32().max(1.0 / 240.0);
            self.vertical_overscroll.last_velocity =
                ((self.vertical_overscroll.offset - previous_offset) / elapsed)
                    .clamp(-MAX_INITIAL_VELOCITY, MAX_INITIAL_VELOCITY);
        }
        self.vertical_overscroll.last_candidate_at = Some(now);
        cx.notify();
    }

    pub(super) fn reset_vertical_overscroll(&mut self) {
        self.stop_vertical_overscroll_animation();
        self.stop_vertical_overscroll_settle_timer();
        self.vertical_overscroll = VerticalOverscroll::default();
    }

    fn schedule_vertical_overscroll_settle(
        &mut self,
        gesture_id: u64,
        immediate: bool,
        cx: &mut Context<Self>,
    ) {
        self.stop_vertical_overscroll_settle_timer();
        self.vertical_overscroll.settle_generation = self
            .vertical_overscroll
            .settle_generation
            .wrapping_add(1)
            .max(1);
        let generation = self.vertical_overscroll.settle_generation;
        let delay = if immediate {
            Duration::ZERO
        } else {
            Duration::from_millis(metrics::VERTICAL_OVERSCROLL_END_DELAY_MS)
        };

        self.vertical_overscroll_settle_timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;
            let _ = this.update(cx, |view, cx| {
                if view.vertical_overscroll.settle_generation == generation
                    && view.vertical_overscroll.ended_gesture_id == gesture_id
                {
                    view.vertical_overscroll_settle_timer = None;
                    view.spring_vertical_overscroll_to_rest(cx);
                }
            });
        }));
    }

    fn spring_vertical_overscroll_to_rest(&mut self, cx: &mut Context<Self>) {
        self.stop_vertical_overscroll_animation();
        if self.vertical_overscroll.offset.abs() <= SPRING_EPSILON {
            self.vertical_overscroll.offset = 0.0;
            self.vertical_overscroll.raw_offset = 0.0;
            self.vertical_overscroll.last_velocity = 0.0;
            self.vertical_overscroll.latest_gesture_id = 0;
            self.vertical_overscroll.ended_gesture_id = 0;
            self.vertical_overscroll.cancelled_gesture_id = 0;
            cx.notify();
            return;
        }

        self.vertical_overscroll.animation_generation = self
            .vertical_overscroll
            .animation_generation
            .wrapping_add(1)
            .max(1);
        let generation = self.vertical_overscroll.animation_generation;
        let initial = SpringState {
            position: self.vertical_overscroll.offset,
            velocity: self.vertical_overscroll.last_velocity,
        };
        let spring = SpringConfig::new(
            metrics::VERTICAL_OVERSCROLL_SPRING_STIFFNESS,
            metrics::VERTICAL_OVERSCROLL_SPRING_DAMPING,
            1.0,
        );

        if cx.reduce_motion() {
            self.finish_vertical_overscroll_spring();
            cx.notify();
            return;
        }

        self.vertical_overscroll_animation = Some(cx.spawn(async move |this, cx| {
            let mut spring_state = initial;
            let mut previous = Instant::now();
            loop {
                cx.background_executor().timer(ANIMATION_INTERVAL).await;
                let now = Instant::now();
                let elapsed = now.duration_since(previous).as_secs_f32().min(0.05);
                previous = now;
                spring_state = spring.step(spring_state, 0.0, elapsed);
                let state = spring_state;
                let keep_running = this
                    .update(cx, |view, cx| {
                        if view.vertical_overscroll.animation_generation != generation {
                            return false;
                        }
                        view.vertical_overscroll.offset = state.position;
                        cx.notify();
                        if spring.is_settled(state, 0.0, SPRING_EPSILON) {
                            view.finish_vertical_overscroll_spring();
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

    fn finish_vertical_overscroll_spring(&mut self) {
        self.vertical_overscroll.offset = 0.0;
        self.vertical_overscroll.raw_offset = 0.0;
        self.vertical_overscroll.last_velocity = 0.0;
        self.vertical_overscroll.last_candidate_at = None;
        self.vertical_overscroll.latest_gesture_id = 0;
        self.vertical_overscroll.ended_gesture_id = 0;
        self.vertical_overscroll.cancelled_gesture_id = 0;
    }

    fn stop_vertical_overscroll_animation(&mut self) {
        self.vertical_overscroll_animation.take();
        self.vertical_overscroll.animation_generation = self
            .vertical_overscroll
            .animation_generation
            .wrapping_add(1)
            .max(1);
    }

    fn stop_vertical_overscroll_settle_timer(&mut self) {
        self.vertical_overscroll_settle_timer.take();
        self.vertical_overscroll.settle_generation = self
            .vertical_overscroll
            .settle_generation
            .wrapping_add(1)
            .max(1);
    }

    fn device_scale(&self) -> f32 {
        self.last_viewport
            .map_or(1.0, |viewport| f32::from_bits(viewport.2))
            .max(0.01)
    }

    fn maximum_overscroll(&self) -> f32 {
        let viewport_height = self.last_viewport.map_or(0.0, |viewport| viewport.1 as f32);
        (viewport_height * metrics::VERTICAL_OVERSCROLL_MAX_DISTANCE_FRACTION)
            .min(metrics::VERTICAL_OVERSCROLL_MAX_DISTANCE)
            .max(0.0)
    }
}

fn is_precise_native_gesture(event: &ScrollWheelEvent) -> bool {
    matches!(event.delta, ScrollDelta::Pixels(_)) && event.has_native_phase
}

fn rubberband(raw_offset: f32, maximum: f32) -> f32 {
    if maximum <= 0.0 {
        return 0.0;
    }
    let distance = raw_offset.abs();
    raw_offset.signum() * maximum * (1.0 - (-distance / maximum).exp())
}

fn raw_offset_from_visual(offset: f32, maximum: f32) -> f32 {
    if maximum <= 0.0 {
        return 0.0;
    }
    let ratio = (offset.abs() / maximum).clamp(0.0, 0.999);
    offset.signum() * -maximum * (1.0 - ratio).ln()
}
