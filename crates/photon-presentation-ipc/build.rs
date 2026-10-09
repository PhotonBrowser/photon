use std::{env, path::PathBuf};

fn main() {
    let source = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("native/PhotonPresentationXpc.m");
    println!("cargo:rerun-if-changed={}", source.display());

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }

    cc::Build::new()
        .file(&source)
        .flag("-fobjc-arc")
        .flag("-fblocks")
        .compile("photon_presentation_xpc");

    for framework in ["Foundation", "Metal", "CoreFoundation"] {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
}
