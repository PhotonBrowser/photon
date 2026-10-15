use gpui::{Context, Task};
use std::time::{Duration, Instant};

use crate::{Entrance, MotionConfig, Transition};

/// Keeps a single value rendered while it animates out after its owner removes it.
///
/// This is a single-slot lifecycle helper. Replacing one present value with
/// another immediately enters the new value; it does not keep both values for
/// a crossfade. Re-entering during exit cancels the pending removal.
pub struct Presence<T> {
    state: PresenceState<T>,
    /// Redraws the owner once the exit ends; dropping it cancels the task.
    expiry: Option<Task<()>>,
}

impl<T> Default for Presence<T> {
    fn default() -> Self {
        Self {
            state: PresenceState::default(),
            expiry: None,
        }
    }
}

impl<T: Clone + 'static> Presence<T> {
    /// Returns what to draw and whether it is entering or exiting.
    pub fn sync<V: 'static>(
        &mut self,
        current: Option<T>,
        entrance: Entrance,
        config: &MotionConfig,
        cx: &mut Context<V>,
    ) -> Option<(T, Transition)> {
        let allowed = config.allows_motion() && !cx.reduce_motion();
        let exit_duration = if allowed {
            entrance.exit_duration(config)
        } else {
            Duration::ZERO
        };
        let now = Instant::now();
        let (display, schedule) = self.state.sync_at(current, now, exit_duration, allowed);

        if schedule.is_some() {
            self.expiry = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(exit_duration).await;
                let _ = this.update(cx, |_, cx| cx.notify());
            }));
        } else if !allowed || matches!(display.as_ref(), Some((_, Transition::Enter))) {
            self.expiry = None;
        }

        display
    }
}

struct PresenceState<T> {
    shown: Option<T>,
    leaving: Option<(T, Instant)>,
    generation: u64,
}

impl<T> Default for PresenceState<T> {
    fn default() -> Self {
        Self {
            shown: None,
            leaving: None,
            generation: 0,
        }
    }
}

impl<T: Clone> PresenceState<T> {
    fn sync_at(
        &mut self,
        current: Option<T>,
        now: Instant,
        exit_duration: Duration,
        allowed: bool,
    ) -> (Option<(T, Transition)>, Option<u64>) {
        if let Some(current) = current {
            self.generation = self.generation.wrapping_add(1);
            self.shown = Some(current.clone());
            self.leaving = None;
            return (Some((current, Transition::Enter)), None);
        }

        if !allowed {
            self.generation = self.generation.wrapping_add(1);
            self.shown = None;
            self.leaving = None;
            return (None, None);
        }

        if let Some(last) = self.shown.take() {
            self.generation = self.generation.wrapping_add(1);
            if exit_duration.is_zero() {
                self.leaving = None;
                return (None, None);
            }
            self.leaving = Some((last, now));
            return (
                self.leaving
                    .as_ref()
                    .map(|(value, _)| (value.clone(), Transition::Exit)),
                Some(self.generation),
            );
        }

        match &self.leaving {
            Some((last, started)) if now.saturating_duration_since(*started) < exit_duration => {
                (Some((last.clone(), Transition::Exit)), None)
            }
            Some(_) => {
                self.generation = self.generation.wrapping_add(1);
                self.leaving = None;
                (None, None)
            }
            None => (None, None),
        }
    }

    #[cfg(test)]
    fn is_exiting(&self, generation: u64) -> bool {
        self.generation == generation && self.leaving.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::PresenceState;
    use crate::Transition;
    use std::time::{Duration, Instant};

    #[test]
    fn exit_stays_present_until_its_deadline() {
        let start = Instant::now();
        let duration = Duration::from_millis(100);
        let mut state = PresenceState::default();
        state.sync_at(Some("menu"), start, duration, true);

        let (display, scheduled) = state.sync_at(None, start, duration, true);
        assert_eq!(display, Some(("menu", Transition::Exit)));
        assert!(scheduled.is_some());
        assert_eq!(
            state.sync_at(None, start + duration, duration, true).0,
            None
        );
    }

    #[test]
    fn reentry_invalidates_the_old_expiry() {
        let start = Instant::now();
        let duration = Duration::from_millis(100);
        let mut state = PresenceState::default();
        state.sync_at(Some("menu"), start, duration, true);
        let (_, old_generation) = state.sync_at(None, start, duration, true);

        let (display, _) = state.sync_at(
            Some("menu"),
            start + Duration::from_millis(50),
            duration,
            true,
        );
        assert_eq!(display, Some(("menu", Transition::Enter)));
        assert!(!state.is_exiting(old_generation.unwrap()));
    }

    #[test]
    fn disabled_motion_releases_exiting_values_immediately() {
        let start = Instant::now();
        let duration = Duration::from_millis(100);
        let mut state = PresenceState::default();
        state.sync_at(Some("menu"), start, duration, true);
        state.sync_at(None, start, duration, true);

        assert_eq!(state.sync_at(None, start, duration, false), (None, None));
        assert!(state.leaving.is_none());
    }
}
