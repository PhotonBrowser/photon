//! Photon-specific GPUI shell and native Engine/presentation adapter.

#[cfg(target_os = "macos")]
mod platform;

/// Run the native Photon browser shell.
#[cfg(target_os = "macos")]
pub fn run() {
    platform::run();
}

/// The native Engine and external Metal presentation path currently targets macOS.
#[cfg(not(target_os = "macos"))]
pub fn run() {
    eprintln!("the direct GPUI-CE Photon path currently supports macOS only");
    std::process::exit(1);
}
