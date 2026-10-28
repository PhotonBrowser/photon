//! Keeping an element drawn while it leaves.

use super::*;

/// Keeps one value mounted through its exit transition.
pub(in super::super) struct Presence<T> {
    shown: Option<T>,
    leaving: Option<(T, Instant)>,
    expiry: Option<gpui::Task<()>>,
}

impl<T> Default for Presence<T> {
    fn default() -> Self {
        Self {
            shown: None,
            leaving: None,
            expiry: None,
        }
    }
}

impl<T: Clone + 'static> Presence<T> {
    pub fn sync<V: 'static>(
        &mut self,
        current: Option<T>,
        entrance: Entrance,
        config: &MotionConfig,
        cx: &mut gpui::Context<V>,
    ) -> Option<(T, Transition)> {
        let allowed = config.allows_motion() && !cx.reduce_motion();
        let duration = if allowed {
            entrance.exit_duration(config)
        } else {
            Duration::ZERO
        };
        let now = Instant::now();

        if let Some(current) = current {
            self.shown = Some(current.clone());
            self.leaving = None;
            self.expiry = None;
            return Some((current, Transition::Enter));
        }

        if !allowed {
            self.shown = None;
            self.leaving = None;
            self.expiry = None;
            return None;
        }

        if let Some(last) = self.shown.take() {
            if duration.is_zero() {
                self.leaving = None;
                return None;
            }
            self.leaving = Some((last.clone(), now));
            self.expiry = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(duration).await;
                let _ = this.update(cx, |_, cx| cx.notify());
            }));
            return Some((last, Transition::Exit));
        }

        match &self.leaving {
            Some((last, started)) if now.saturating_duration_since(*started) < duration => {
                Some((last.clone(), Transition::Exit))
            }
            Some(_) => {
                self.leaving = None;
                self.expiry = None;
                None
            }
            None => None,
        }
    }
}
