use std::{env, path::PathBuf};

const BACKGROUND_NOISE_SIZE: u32 = 1_024;

fn main() {
    println!("cargo:rerun-if-env-changed=PHOTON_ENGINE_BUILD_DIR");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/platform/ui/window/background_noise.rs");

    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    generate_background_noise(&out);

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if !matches!(target_os.as_str(), "macos" | "linux" | "windows") {
        return;
    }

    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("../..")
        .canonicalize()
        .unwrap();
    let ui_only = env::var_os("CARGO_FEATURE_UI_ONLY").is_some();
    if !ui_only {
        let engine_build = env::var_os("PHOTON_ENGINE_BUILD_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("build/engine-release"));
        let generated = engine_build.join("Libraries");
        let fallback = out.join("include/LibPhotonEmbedder/Export.h");
        if !generated.join("LibPhotonEmbedder/Export.h").is_file() {
            std::fs::create_dir_all(fallback.parent().unwrap()).unwrap();
            std::fs::write(&fallback, "#pragma once\n#define PHOTONEMBEDDER_API\n").unwrap();
        }
        let mut bridge = cc::Build::new();
        bridge
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
            });
        if target_os == "macos" {
            bridge.include(root.join("native/embedder/macos"));
            println!(
                "cargo:rerun-if-changed={}",
                root.join("native/embedder/macos/SystemClipboard.h")
                    .display()
            );
        }
        bridge.compile("photon_embedder_bridge_direct");
        for source in [
            "native/embedder/PhotonEmbedderBridge.cpp",
            "native/embedder/PhotonEmbedderBridge.h",
            "Engine/Libraries/LibPhotonEmbedder/include/LibPhotonEmbedder/PhotonEmbedder.h",
            "Engine/Libraries/LibPhotonEmbedder/include/LibPhotonEmbedder/PhotonAutomation.h",
        ] {
            println!("cargo:rerun-if-changed={}", root.join(source).display());
        }
        let link_directory = if engine_build.join("lib64").exists() {
            engine_build.join("lib64")
        } else {
            engine_build.join("lib")
        };
        println!(
            "cargo:rustc-link-search=native={}",
            link_directory.display()
        );
        if target_os == "windows" {
            println!(
                "cargo:rustc-link-search=native={}",
                engine_build.join("bin").display()
            );
        }
        if target_os == "windows" {
            // CMake produces an MSVC import library for the engine DLL.
            println!("cargo:rustc-link-lib=static=lagom-photonembedder");
        } else {
            println!("cargo:rustc-link-lib=dylib=lagom-photonembedder");
        }
    }
    if target_os != "macos" {
        return;
    }
    for source in [
        "native/embedder/macos/DefaultBrowser.mm",
        "native/embedder/macos/ReducedMotionObserver.mm",
        "native/embedder/macos/SystemClipboard.h",
        "native/embedder/macos/SystemClipboard.mm",
        "native/embedder/macos/WindowObserver.mm",
    ] {
        println!("cargo:rerun-if-changed={}", root.join(source).display());
    }
    cc::Build::new()
        .cpp(true)
        .file(root.join("native/embedder/macos/DefaultBrowser.mm"))
        .file(root.join("native/embedder/macos/ReducedMotionObserver.mm"))
        .file(root.join("native/embedder/macos/SystemClipboard.mm"))
        .file(root.join("native/embedder/macos/WindowObserver.mm"))
        .std("c++20")
        .flag("-fobjc-arc")
        .flag("-fblocks")
        .include(root.join("native/embedder"))
        .include(root.join("native/embedder/macos"))
        .compile("photon_native_macos");
    for framework in ["AppKit", "Foundation", "Metal", "CoreFoundation"] {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
}

fn generate_background_noise(out: &std::path::Path) {
    let mut pixels =
        Vec::with_capacity(BACKGROUND_NOISE_SIZE as usize * BACKGROUND_NOISE_SIZE as usize * 4);
    for y in 0..BACKGROUND_NOISE_SIZE {
        for x in 0..BACKGROUND_NOISE_SIZE {
            let mut value = x
                .wrapping_mul(0x9e37_79b9)
                .wrapping_add(y.wrapping_mul(0x85eb_ca6b))
                .wrapping_add(0xc2b2_ae35);
            value ^= value >> 16;
            value = value.wrapping_mul(0x7feb_352d);
            value ^= value >> 15;
            let grain = (value >> 24) as u8;
            pixels.extend_from_slice(&[grain, grain, grain, u8::MAX]);
        }
    }
    std::fs::write(out.join("background-noise.rgba"), pixels)
        .expect("write precomputed background noise texture");
}
