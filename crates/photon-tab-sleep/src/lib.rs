//! Browser tab sleep modes and the idle policy that controls them.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// How quickly Photon stops rendering inactive pages while keeping them live.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TabSleepMode {
    /// Do not automatically mark inactive pages as sleeping.
    Off,
    /// Sleep pages after one minute in the background.
    #[default]
    Balanced,
    /// Sleep pages after five seconds in the background.
    Maximum,
}

const BALANCED_IDLE: Duration = Duration::from_secs(60);
const MAXIMUM_IDLE: Duration = Duration::from_secs(5);

/// Returns how long an inactive page stays awake. `None` disables sleeping.
pub fn idle_timeout(mode: TabSleepMode) -> Option<Duration> {
    match mode {
        TabSleepMode::Off => None,
        TabSleepMode::Maximum => Some(MAXIMUM_IDLE),
        TabSleepMode::Balanced => Some(BALANCED_IDLE),
    }
}

#[cfg(test)]
mod tests {
    use super::{TabSleepMode, idle_timeout};
    use std::time::Duration;

    #[test]
    fn off_disables_automatic_sleep() {
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
}
