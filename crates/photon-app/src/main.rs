#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("the direct GPUI-CE Photon path currently supports macOS only");
    std::process::exit(1);
}

#[cfg(target_os = "macos")]
mod platform;

#[cfg(target_os = "macos")]
fn main() {
    platform::run();
}
