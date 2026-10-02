fn main() {
    if !cfg!(target_os = "macos") {
        return;
    }
    let root =
        std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    cc::Build::new()
        .file(root.join("native/presentation/PhotonPresentationXpc.m"))
        .flag("-fobjc-arc")
        .flag("-fblocks")
        .compile("photon_presentation_xpc_broker");
    for framework in ["Foundation", "Metal", "CoreFoundation"] {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
    println!(
        "cargo:rerun-if-changed={}",
        root.join("native/presentation/PhotonPresentationXpc.m")
            .display()
    );
}
