//! Browser tab sleep modes and the idle policy that controls them.

use serde::{Deserialize, Serialize};
use std::time::Duration;

const WARNING_CLEANUP_BATCH: usize = 4;
const CRITICAL_CLEANUP_BATCH: usize = 8;

/// The operating system's current memory pressure level.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MemoryPressure {
    #[default]
    Normal,
    Warning,
    Critical,
}

impl MemoryPressure {
    /// Bounds how many hidden page sessions are cleaned in one maintenance pass.
    pub const fn cleanup_batch(self) -> usize {
        match self {
            Self::Normal => 0,
            Self::Warning => WARNING_CLEANUP_BATCH,
            Self::Critical => CRITICAL_CLEANUP_BATCH,
        }
    }

    /// Allows reclaiming eligible hidden page sessions when the system is under critical pressure.
    pub const fn should_discard_hidden_tabs(self) -> bool {
        matches!(self, Self::Critical)
    }
}

/// Controls when Photon discards eligible page processes.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TabSleepMode {
    /// Keep pages loaded unless memory pressure requires a discard.
    Off,
    /// Discard pages after thirty seconds out of view, or five minutes without system input.
    #[default]
    Balanced,
    /// Discard pages after five seconds out of view, or one minute without system input.
    Maximum,
}

const BALANCED_IDLE: Duration = Duration::from_secs(30);
const MAXIMUM_IDLE: Duration = Duration::from_secs(5);
const BALANCED_ACTIVE_IDLE: Duration = Duration::from_secs(5 * 60);
const MAXIMUM_ACTIVE_IDLE: Duration = Duration::from_secs(60);

/// Returns how long a page out of view stays loaded. `None` disables idle discard.
pub fn idle_timeout(mode: TabSleepMode) -> Option<Duration> {
    match mode {
        TabSleepMode::Off => None,
        TabSleepMode::Maximum => Some(MAXIMUM_IDLE),
        TabSleepMode::Balanced => Some(BALANCED_IDLE),
    }
}

/// Returns how long the selected page stays loaded without system input.
/// `None` disables idle discard.
pub fn active_idle_timeout(mode: TabSleepMode) -> Option<Duration> {
    match mode {
        TabSleepMode::Off => None,
        TabSleepMode::Maximum => Some(MAXIMUM_ACTIVE_IDLE),
        TabSleepMode::Balanced => Some(BALANCED_ACTIVE_IDLE),
    }
}

/// How much longer a page can stay loaded before its idle timeout expires.
/// Active visible pages require both tab inactivity and system inactivity.
pub fn idle_discard_delay(
    mode: TabSleepMode,
    active_visible: bool,
    since_tab_activity: Duration,
    system_idle: Option<Duration>,
) -> Option<Duration> {
    if active_visible {
        let timeout = active_idle_timeout(mode)?;
        let system_idle = system_idle?;
        Some(
            timeout
                .saturating_sub(system_idle)
                .max(timeout.saturating_sub(since_tab_activity)),
        )
    } else {
        let timeout = idle_timeout(mode)?;
        Some(timeout.saturating_sub(since_tab_activity))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MemoryPressure, TabSleepMode, active_idle_timeout, idle_discard_delay, idle_timeout,
    };
    use std::time::Duration;

    #[test]
    fn off_disables_automatic_discard() {
        assert_eq!(idle_timeout(TabSleepMode::Off), None);
    }

    #[test]
    fn maximum_uses_five_seconds() {
        assert_eq!(
            idle_timeout(TabSleepMode::Maximum),
            Some(Duration::from_secs(5))
        );
    }

    #[test]
    fn balanced_uses_thirty_seconds() {
        assert_eq!(
            idle_timeout(TabSleepMode::Balanced),
            Some(Duration::from_secs(30))
        );
    }

    #[test]
    fn active_idle_timeout_is_longer_than_background_timeout() {
        assert_eq!(active_idle_timeout(TabSleepMode::Off), None);
        assert_eq!(
            active_idle_timeout(TabSleepMode::Balanced),
            Some(Duration::from_secs(5 * 60))
        );
        assert_eq!(
            active_idle_timeout(TabSleepMode::Maximum),
            Some(Duration::from_secs(60))
        );
    }

    #[test]
    fn background_discard_uses_time_out_of_view() {
        assert_eq!(
            idle_discard_delay(TabSleepMode::Balanced, false, Duration::from_secs(10), None,),
            Some(Duration::from_secs(20))
        );
        assert_eq!(
            idle_discard_delay(TabSleepMode::Balanced, false, Duration::from_secs(40), None,),
            Some(Duration::ZERO)
        );
    }

    #[test]
    fn active_discard_waits_for_both_tab_and_system_idle() {
        assert_eq!(
            idle_discard_delay(
                TabSleepMode::Balanced,
                true,
                Duration::from_secs(100),
                Some(Duration::from_secs(300)),
            ),
            Some(Duration::from_secs(200))
        );
        assert_eq!(
            idle_discard_delay(
                TabSleepMode::Balanced,
                true,
                Duration::from_secs(400),
                Some(Duration::from_secs(120)),
            ),
            Some(Duration::from_secs(180))
        );
        assert_eq!(
            idle_discard_delay(TabSleepMode::Balanced, true, Duration::ZERO, None),
            None
        );
    }

    #[test]
    fn pressure_cleanup_is_bounded_and_reacts_faster_when_urgent() {
        assert_eq!(MemoryPressure::Normal.cleanup_batch(), 0);
        assert_eq!(MemoryPressure::Warning.cleanup_batch(), 4);
        assert_eq!(MemoryPressure::Critical.cleanup_batch(), 8);
        assert!(!MemoryPressure::Normal.should_discard_hidden_tabs());
        assert!(!MemoryPressure::Warning.should_discard_hidden_tabs());
        assert!(MemoryPressure::Critical.should_discard_hidden_tabs());
    }
}
