//! Photon-specific GPUI shell and native Engine/presentation adapter.

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
mod platform;

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub use platform::show_alert;

/// Native AppKit views that participate in GPUI layout on macOS.
#[cfg(target_os = "macos")]
pub use platform::{AppKitButton, AppKitButtonStyle, AppKitTabBar, AppKitView, AppKitViewElement};

/// Run the native Photon browser shell.
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
pub fn run() {
    platform::run();
}

/// Other targets do not have Photon shell adapters.
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub fn run() {
    eprintln!("the direct GPUI-CE Photon shell currently supports macOS, Linux, and Windows");
    std::process::exit(1);
}
