mod discovery;
mod process;
mod process_info;
mod resource;
mod window;

use std::path::PathBuf;

pub use discovery::{BrowserDiscoveryError, MacOSBrowserDiscovery};
pub use process::{MacOSProcessDriver, ManagedProcessDiagnostics};
pub use resource::MacOSResourceCollector;
pub use window::MacOSWindowObserver;

pub fn default_session_store_path() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
    Ok(PathBuf::from(home)
        .join("Library/Application Support/Velocity")
        .join("managed-sessions.json"))
}
