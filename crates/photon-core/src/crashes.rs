//! Recovering from crashed pages and Engine services.

use std::time::{Duration, Instant};

/// An Engine service process, restarted by the Engine when it stops.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EngineService {
    /// Draws pages to the screen.
    Compositor,
    /// Loads everything from the network.
    Network,
}

/// What to do about a page whose process crashed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CrashResponse {
    /// Reload the page in a fresh process.
    Reload,
    /// The page crashed again soon after recovering; reloading it again would
    /// likely loop, so leave reloading to the user.
    GiveUp,
}

/// A crash this soon after the page recovered counts as crashing again.
const REPEAT_WINDOW: Duration = Duration::from_secs(10);

/// One page's crash history.
#[derive(Clone, Debug, Default)]
pub struct PageCrashes {
    /// A reload after a crash has not yet shown the page.
    recovering: bool,
    recovered_at: Option<Instant>,
}

impl PageCrashes {
    /// The page's process crashed at `now`.
    pub fn crashed(&mut self, now: Instant) -> CrashResponse {
        let repeated = self.recovering
            || self
                .recovered_at
                .is_some_and(|recovered| now.duration_since(recovered) < REPEAT_WINDOW);
        self.recovering = !repeated;
        if repeated {
            CrashResponse::GiveUp
        } else {
            CrashResponse::Reload
        }
    }

    /// The user asked to reload a page that kept crashing.
    pub fn reload_requested(&mut self) {
        self.recovering = true;
    }

    /// The page that replaced the crashed one is showing again.
    pub fn recovered(&mut self, now: Instant) {
        self.recovering = false;
        self.recovered_at = Some(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_first_crash_reloads() {
        let mut crashes = PageCrashes::default();
        assert_eq!(crashes.crashed(Instant::now()), CrashResponse::Reload);
    }

    #[test]
    fn crashing_while_recovering_gives_up() {
        let mut crashes = PageCrashes::default();
        let now = Instant::now();
        crashes.crashed(now);
        assert_eq!(crashes.crashed(now), CrashResponse::GiveUp);
    }

    #[test]
    fn crashing_soon_after_recovering_gives_up() {
        let mut crashes = PageCrashes::default();
        let now = Instant::now();
        crashes.crashed(now);
        crashes.recovered(now);
        assert_eq!(
            crashes.crashed(now + Duration::from_secs(2)),
            CrashResponse::GiveUp
        );
    }

    #[test]
    fn crashing_long_after_recovering_reloads_again() {
        let mut crashes = PageCrashes::default();
        let now = Instant::now();
        crashes.crashed(now);
        crashes.recovered(now);
        assert_eq!(
            crashes.crashed(now + REPEAT_WINDOW + Duration::from_secs(1)),
            CrashResponse::Reload
        );
    }

    #[test]
    fn a_requested_reload_counts_as_recovering() {
        let mut crashes = PageCrashes::default();
        let now = Instant::now();
        crashes.crashed(now);
        crashes.crashed(now);
        crashes.reload_requested();
        assert_eq!(crashes.crashed(now), CrashResponse::GiveUp);
    }
}
