//! Refresh-rate fallback for targets without a native display query adapter.

const FALLBACK_REFRESH_RATE: f64 = 60.0;

pub(in crate::platform) fn refresh_rate(_: u64) -> f64 {
    FALLBACK_REFRESH_RATE
}

pub(in crate::platform) fn known_refresh_rate(_: u64) -> Option<f64> {
    None
}
