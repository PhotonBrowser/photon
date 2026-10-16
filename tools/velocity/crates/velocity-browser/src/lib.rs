//! Browser metadata, discovery, registry, and managed process lifecycle.

mod automation;
mod cdp;
mod lifecycle;
mod model;
#[cfg(target_os = "macos")]
mod photon;
mod registry;
mod session_store;
mod window;

#[cfg(target_os = "macos")]
pub mod macos;

pub use automation::{
    AutomationProtocol, BrowserAutomationAdapter, BrowserCapabilities, browser_capabilities,
};
pub use cdp::{AutomationError, ChromiumDevTools, JetStreamPageStatus, SpeedometerPageStatus};
pub use lifecycle::{
    BrowserLifecycleManager, CloseOutcome, LifecycleError, ManagedSession, ProcessDriver,
    ProcessError, ProcessIdentity, ProcessLaunchTiming, ProcessState,
};
pub use model::{
    BrowserEngine, BrowserId, BrowserInstallation, BrowserMetadata, IdentifierError, InstallationId,
};
#[cfg(target_os = "macos")]
pub use photon::{
    PhotonAutomationClient, PhotonAutomationError, PhotonJetStreamResult, PhotonParentIdentity,
    PhotonProcessRegistry, PhotonRegisteredProcess, PhotonSpeedometerResult, PhotonTab,
    PhotonWindow, parse_jetstream_result_title, parse_speedometer_result_title,
};
pub use registry::{BrowserRegistry, RegistryError};
pub use session_store::{FileSessionStore, SessionStore, SessionStoreError, SessionStoreGuard};
pub use window::{BrowserWindowObserver, WindowObservation, WindowObservationError};
