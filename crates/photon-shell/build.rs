use std::{env, path::PathBuf};

fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("../..")
        .canonicalize()
        .unwrap();
    let engine_build = env::var_os("PHOTON_ENGINE_BUILD_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("build/engine-debug"));
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let generated = engine_build.join("Libraries");
    let fallback = out.join("include/LibPhotonEmbedder/Export.h");
    if !generated.join("LibPhotonEmbedder/Export.h").is_file() {
        std::fs::create_dir_all(fallback.parent().unwrap()).unwrap();
        std::fs::write(&fallback, "#pragma once\n#define PHOTONEMBEDDER_API\n").unwrap();
    }
    cc::Build::new()
        .cpp(true)
        .std("c++20")
        .file(root.join("native/embedder/PhotonEmbedderBridge.cpp"))
        .include(root.join("native/embedder"))
        .include(root.join("Engine/Libraries/LibPhotonEmbedder/include"))
        .include(root.join("Engine/Libraries"))
        .include(if generated.join("LibPhotonEmbedder/Export.h").is_file() {
            generated
        } else {
            out.join("include")
        })
        .compile("photon_embedder_bridge_direct");
    println!(
        "cargo:rerun-if-changed={}",
        root.join("native/embedder/PhotonEmbedderBridge.cpp")
            .display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        engine_build.join("lib64").display()
    );
    if !engine_build.join("lib64").exists() {
        println!(
            "cargo:rustc-link-search=native={}",
            engine_build.join("lib").display()
        );
    }
    println!("cargo:rustc-link-lib=dylib=lagom-photonembedder");
    if cfg!(target_os = "macos") {
        cc::Build::new()
            .file(root.join("native/presentation/PhotonPresentationXpc.m"))
            .flag("-fobjc-arc")
            .flag("-fblocks")
            .compile("photon_presentation_xpc_direct");
        for framework in ["Foundation", "Metal", "CoreFoundation"] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
    }
    println!("cargo:rerun-if-env-changed=PHOTON_ENGINE_BUILD_DIR");
}
