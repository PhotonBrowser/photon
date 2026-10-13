//! Browser tab sleep modes and the idle policy that controls them.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// How aggressively Photon marks quiet inactive pages as sleeping.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TabSleepMode {
    /// Do not automatically mark inactive pages as sleeping.
    Off,
    /// Wait longer when few pages are idle and shorten the wait as they pile up.
    #[default]
    Balanced,
    /// Mark an idle page as sleeping after five seconds.
    Maximum,
}

const MAXIMUM_IDLE: Duration = Duration::from_secs(5);
const BALANCED_LONGEST_IDLE_SECONDS: u64 = 5 * 60;
const BALANCED_SHORTEST_IDLE_SECONDS: u64 = 60;
const BALANCED_MAX_CANDIDATES: u64 = 10;

/// Returns how long an eligible inactive page stays awake, based on the
/// number of eligible inactive pages in the window. A `None` result disables
/// automatic sleeping.
pub fn idle_timeout(mode: TabSleepMode, eligible_inactive_pages: usize) -> Option<Duration> {
    match mode {
        TabSleepMode::Off => None,
        TabSleepMode::Maximum => Some(MAXIMUM_IDLE),
        TabSleepMode::Balanced => {
            let pressure = (eligible_inactive_pages as u64)
                .saturating_sub(1)
                .min(BALANCED_MAX_CANDIDATES - 1);
            let range = BALANCED_LONGEST_IDLE_SECONDS - BALANCED_SHORTEST_IDLE_SECONDS;
            let reduction = range * pressure / (BALANCED_MAX_CANDIDATES - 1);
            Some(Duration::from_secs(
                BALANCED_LONGEST_IDLE_SECONDS - reduction,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TabSleepMode, idle_timeout};
    use std::time::Duration;

    #[test]
    fn off_disables_automatic_sleep() {
        assert_eq!(idle_timeout(TabSleepMode::Off, 12), None);
    }

    #[test]
    fn maximum_uses_five_seconds() {
        assert_eq!(
            idle_timeout(TabSleepMode::Maximum, 0),
            Some(Duration::from_secs(5))
        );
        assert_eq!(
            idle_timeout(TabSleepMode::Maximum, 12),
            Some(Duration::from_secs(5))
        );
    }

    #[test]
    fn balanced_shortens_with_idle_tab_pressure_and_has_a_floor() {
        assert_eq!(
            idle_timeout(TabSleepMode::Balanced, 0),
            Some(Duration::from_secs(5 * 60))
        );
        assert_eq!(
            idle_timeout(TabSleepMode::Balanced, 1),
            Some(Duration::from_secs(5 * 60))
        );
        assert_eq!(
            idle_timeout(TabSleepMode::Balanced, 10),
            Some(Duration::from_secs(60))
        );
        assert_eq!(
            idle_timeout(TabSleepMode::Balanced, 20),
            Some(Duration::from_secs(60))
        );
    }
}
