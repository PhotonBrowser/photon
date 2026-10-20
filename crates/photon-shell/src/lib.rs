//! Photon-specific GPUI shell and native Engine/presentation adapter.

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod platform;

#[cfg(target_os = "macos")]
pub use platform::show_alert;

/// Run the native Photon browser shell.
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn run() {
    platform::run();
}

/// Other targets do not have Photon shell adapters.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn run() {
    eprintln!("the direct GPUI-CE Photon shell currently supports Linux and macOS targets only");
    std::process::exit(1);
}
