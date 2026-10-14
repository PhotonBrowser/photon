/// One sample from a platform window observer for a managed process session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WindowObservation {
    ProcessExited,
    OwnershipUncertain,
    VisibleWindows(Vec<u32>),
}

/// Reads visible windows belonging to one verified Velocity process session.
pub trait BrowserWindowObserver: Send + Sync {
    fn observe(
        &self,
        process_id: u32,
        process_session_id: u32,
        executable_path: &std::path::Path,
    ) -> Result<WindowObservation, WindowObservationError>;
}

/// Failure to inspect windows without making an ownership assumption.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowObservationError(pub String);

impl std::fmt::Display for WindowObservationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for WindowObservationError {}
