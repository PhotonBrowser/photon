//! macOS GPUI-CE shell: wires UI, Engine, and native presentation modules together.

mod display;
mod engine;
mod ffi;
mod presentation;
mod presentation_xpc;
mod ui;
mod window_observer;
mod window_settings;

fn trace(args: std::fmt::Arguments<'_>) {
    if std::env::var_os("PHOTON_VERBOSE").is_some() {
        eprintln!("[PhotonWebView/GPUI-CE] {args}");
    }
}

pub use ui::run;
