//! macOS GPUI-CE shell: wires UI, Engine, and native presentation modules together.

mod display;
mod engine;
mod ffi;
mod presentation;
mod ui;
mod window_observer;
mod window_settings;

fn trace(args: std::fmt::Arguments<'_>) {
    // Read once: traces run on every frame, including in Metal completion handlers.
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *ENABLED.get_or_init(|| std::env::var_os("PHOTON_VERBOSE").is_some()) {
        eprintln!("[PhotonWebView/GPUI-CE] {args}");
    }
}

pub use ui::run;
