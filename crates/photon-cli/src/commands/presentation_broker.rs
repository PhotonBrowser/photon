//! Launchd registration for the Photon-owned macOS IOSurface broker.

use std::path::Path;
use std::process::Command;

use crate::support::{invoke, output, stage};

pub(super) fn ensure(root: &Path, verbose: bool) -> Result<String, String> {
    let root = root.canonicalize().map_err(|error| error.to_string())?;
    let (service, domain, target) = service_location(&root)?;
    if service_is_registered(&target) {
        if verbose {
            println!("Photon presentation broker: {service} (already registered)");
        }
        return Ok(service);
    }

    stage("Build Photon presentation broker");
    invoke(
        "cargo",
        &["build", "-p", "photon-presentation-broker"],
        &root,
        verbose,
    )?;
    let built = root.join("target/debug/photon-presentation-broker");
    if !built.is_file() {
        return Err(format!(
            "Photon presentation broker was not built at {}",
            built.display()
        ));
    }

    let directory = std::env::temp_dir().join(format!("photon-presentation-{service}"));
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let executable = directory.join("presentation_broker");
    std::fs::copy(&built, &executable).map_err(|error| {
        format!(
            "could not stage Photon presentation broker {}: {error}",
            executable.display()
        )
    })?;
    let plist = directory.join(format!("{service}.plist"));
    let stderr = directory.join(format!("{service}.err"));
    let plist_text = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\"><dict><key>Label</key><string>{service}</string>\n\
         <key>ProgramArguments</key><array><string>{}</string><string>{service}</string></array>\n\
         <key>MachServices</key><dict><key>{service}</key><true/><key>{service}.iosurface</key><true/></dict>\n\
         <key>RunAtLoad</key><true/><key>StandardErrorPath</key><string>{}</string></dict></plist>\n",
        xml_escape(&executable.to_string_lossy()),
        xml_escape(&stderr.to_string_lossy())
    );
    std::fs::write(&plist, plist_text).map_err(|error| error.to_string())?;

    stage("Start Photon presentation broker");
    let result = Command::new("launchctl")
        .args(["bootstrap", &domain])
        .arg(&plist)
        .output()
        .map_err(|error| format!("cannot start launchctl: {error}"))?;
    if !result.status.success() && !service_is_registered(&target) {
        return Err(format!(
            "could not start Photon presentation broker: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        ));
    }
    if !service_is_registered(&target) {
        return Err(format!(
            "Photon presentation broker {service} is missing from {domain}"
        ));
    }
    println!("Photon presentation broker: {service}");
    Ok(service)
}

pub(super) fn clean(root: &Path) -> Result<(), String> {
    let root = root.canonicalize().map_err(|error| error.to_string())?;
    let (service, _, target) = service_location(&root)?;
    if service_is_registered(&target) {
        let status = Command::new("launchctl")
            .args(["bootout", &target])
            .status()
            .map_err(|error| format!("cannot stop Photon presentation broker: {error}"))?;
        if !status.success() {
            return Err(format!(
                "could not stop Photon presentation broker {target}"
            ));
        }
    }
    let directory = std::env::temp_dir().join(format!("photon-presentation-{service}"));
    if directory.exists() {
        std::fs::remove_dir_all(directory).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn service_location(root: &Path) -> Result<(String, String, String), String> {
    let uid = output("id", &["-u"], root)?.trim().to_owned();
    if uid.is_empty() || !uid.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!("invalid user ID for presentation broker: {uid}"));
    }
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in root.as_os_str().as_encoded_bytes() {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
    }
    let service = format!("org.photonbrowser.photon.presentation.u{uid}.h{hash:016x}.gpui-ce");
    let domain = format!("gui/{uid}");
    let target = format!("{domain}/{service}");
    Ok((service, domain, target))
}

fn service_is_registered(target: &str) -> bool {
    Command::new("launchctl")
        .args(["print", target])
        .output()
        .is_ok_and(|result| result.status.success())
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
