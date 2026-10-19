//! Browser tab sleep modes and the idle policy that controls them.

use serde::{Deserialize, Serialize};
use std::time::Duration;

const WARNING_CLEANUP_BATCH: usize = 2;
const CRITICAL_CLEANUP_BATCH: usize = 4;

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

/// How long inactive pages stay loaded before Photon discards their page process.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TabSleepMode {
    /// Keep inactive pages loaded unless memory pressure requires a discard.
    Off,
    /// Discard eligible pages after one minute in the background.
    #[default]
    Balanced,
    /// Discard eligible pages after five seconds in the background.
    Maximum,
}

const BALANCED_IDLE: Duration = Duration::from_secs(60);
const MAXIMUM_IDLE: Duration = Duration::from_secs(5);

/// Returns how long an inactive page stays loaded. `None` disables idle discard.
pub fn idle_timeout(mode: TabSleepMode) -> Option<Duration> {
    match mode {
        TabSleepMode::Off => None,
        TabSleepMode::Maximum => Some(MAXIMUM_IDLE),
        TabSleepMode::Balanced => Some(BALANCED_IDLE),
    }
}

#[cfg(test)]
mod tests {
    use super::{MemoryPressure, TabSleepMode, idle_timeout};
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
    fn balanced_uses_one_minute() {
        assert_eq!(
            idle_timeout(TabSleepMode::Balanced),
            Some(Duration::from_secs(60))
        );
    }

    #[test]
    fn pressure_cleanup_is_bounded_and_reacts_faster_when_urgent() {
        assert_eq!(MemoryPressure::Normal.cleanup_batch(), 0);
        assert_eq!(MemoryPressure::Warning.cleanup_batch(), 2);
        assert_eq!(MemoryPressure::Critical.cleanup_batch(), 4);
        assert!(!MemoryPressure::Normal.should_discard_hidden_tabs());
        assert!(!MemoryPressure::Warning.should_discard_hidden_tabs());
        assert!(MemoryPressure::Critical.should_discard_hidden_tabs());
    }
}
