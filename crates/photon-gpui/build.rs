use std::env;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_ENGINE");
    if env::var_os("CARGO_FEATURE_ENGINE").is_none() {
        return;
    }

    let root =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory")).join("../..");
    let root = root.canonicalize().expect("Photon repository root");
    let engine = root.join("Engine");
    let build = env::var_os("PHOTON_ENGINE_BUILD_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("build/engine-debug"));
    let generated_root = build.join("Libraries");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
    let fallback_header = out.join("include/LibPhotonEmbedder/Export.h");
    if !generated_root.join("LibPhotonEmbedder/Export.h").is_file() {
        std::fs::create_dir_all(fallback_header.parent().expect("header parent"))
            .expect("create fallback include directory");
        std::fs::write(
            &fallback_header,
            "#pragma once\n#define PHOTONEMBEDDER_API\n",
        )
        .expect("write fallback export header");
    }

    cc::Build::new()
        .cpp(true)
        .std("c++20")
        .file(root.join("native/gpui/PhotonEmbedderBridge.cpp"))
        .include(root.join("native/gpui"))
        .include(engine.join("Libraries/LibPhotonEmbedder/include"))
        .include(engine.join("Libraries"))
        .include(
            if generated_root.join("LibPhotonEmbedder/Export.h").is_file() {
                generated_root.clone()
            } else {
                out.join("include")
            },
        )
        .compile("photon_embedder_bridge");

    let library_dir = find_engine_library_dir(&build);
    println!("cargo:rustc-link-search=native={}", library_dir.display());
    println!("cargo:rustc-link-lib=dylib=lagom-photonembedder");
    if cfg!(target_os = "linux") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", library_dir.display());
    }
    println!("cargo:rerun-if-changed=native/gpui/PhotonEmbedderBridge.cpp");
    println!("cargo:rerun-if-changed=native/gpui/PhotonEmbedderBridge.h");
    println!(
        "cargo:rerun-if-changed=Engine/Libraries/LibPhotonEmbedder/include/LibPhotonEmbedder/PhotonEmbedder.h"
    );
    println!("cargo:rerun-if-env-changed=PHOTON_ENGINE_BUILD_DIR");
}

fn find_engine_library_dir(build: &Path) -> PathBuf {
    for candidate in [build.join("lib64"), build.join("lib")] {
        if candidate.exists() {
            return candidate;
        }
    }
    build.join("lib64")
}
