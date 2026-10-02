#[cfg(target_os = "macos")]
fn main() {
    unsafe extern "C" {
        fn photon_presentation_xpc_run_service(name: *const std::ffi::c_char) -> i32;
    }
    let Some(name) = std::env::args().nth(1) else {
        eprintln!("usage: photon-presentation-broker SERVICE_NAME");
        std::process::exit(2);
    };
    let Ok(name) = std::ffi::CString::new(name) else {
        std::process::exit(2);
    };
    let status = unsafe { photon_presentation_xpc_run_service(name.as_ptr()) };
    if status != 0 {
        std::process::exit(status);
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Photon presentation broker is macOS-only");
    std::process::exit(1);
}
