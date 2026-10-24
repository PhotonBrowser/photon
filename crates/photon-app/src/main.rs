#[cfg(all(target_os = "macos", feature = "hot-reload"))]
fn main() {
    if let Err(error) = enter_development_bundle() {
        eprintln!("could not launch Photon from its development app bundle: {error}");
        std::process::exit(1);
    }
    photon_shell::run();
}

#[cfg(not(all(target_os = "macos", feature = "hot-reload")))]
fn main() {
    photon_shell::run();
}

#[cfg(all(target_os = "macos", feature = "hot-reload"))]
fn enter_development_bundle() -> std::io::Result<()> {
    use std::{env, fs, os::unix::process::CommandExt, path::PathBuf, process::Command};

    const BUNDLE_EXECUTABLE: &str = "PHOTON_HOT_RELOAD_BUNDLE_EXECUTABLE";
    const BUNDLE_PATH: &str = "PHOTON_HOT_RELOAD_BUNDLE";
    const REGISTER_BUNDLE: &str = "PHOTON_HOT_RELOAD_REGISTER_BUNDLE";
    const LSREGISTER: &str = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";

    let Some(executable) = env::var_os(BUNDLE_EXECUTABLE).map(PathBuf::from) else {
        return Ok(());
    };
    let Some(bundle) = env::var_os(BUNDLE_PATH).map(PathBuf::from) else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "hot-reload bundle path is missing",
        ));
    };

    let current = env::current_exe()?;
    let parent = executable.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "hot-reload bundle executable has no parent directory",
        )
    })?;
    fs::create_dir_all(parent)?;

    // Dioxus replaces the binary in its build directory on full rebuilds. Copy
    // each version into the prepared app bundle, then exec it there so macOS
    // associates the process with Photon.app. exec keeps Dioxus's PID and pipes.
    let executable_name = executable.file_name().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "hot-reload bundle executable has no file name",
        )
    })?;
    let staged = parent.join(format!(
        ".{}.hot-reload.new",
        executable_name.to_string_lossy()
    ));
    fs::copy(&current, &staged)?;
    fs::rename(&staged, &executable)?;

    if env::var_os(REGISTER_BUNDLE).is_some() {
        let registered = Command::new(LSREGISTER)
            .arg("-f")
            .arg(&bundle)
            .status()
            .is_ok_and(|status| status.success());
        if !registered {
            eprintln!(
                "warning: macOS did not register {}; it cannot be the default browser",
                bundle.display()
            );
        }
    }

    let error = Command::new(executable)
        .args(env::args_os().skip(1))
        .env_remove(BUNDLE_EXECUTABLE)
        .env_remove(BUNDLE_PATH)
        .env_remove(REGISTER_BUNDLE)
        .exec();
    Err(error)
}
