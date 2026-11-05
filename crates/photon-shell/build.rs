use std::{env, path::PathBuf};

const BACKGROUND_NOISE_SIZE: u32 = 880;

fn main() {
    println!("cargo:rerun-if-env-changed=PHOTON_ENGINE_BUILD_DIR");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/zen-grain-bg.png");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    generate_plugin_registry(&out);
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
    if ui_only {
        let stubs = root.join("native/embedder/PhotonEmbedderUiOnlyStubs.cpp");
        println!("cargo:rerun-if-changed={}", stubs.display());
        cc::Build::new()
            .cpp(true)
            .std("c++20")
            .file(&stubs)
            .include(root.join("native/embedder"))
            .compile("photon_embedder_ui_only_stubs");
    } else {
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

fn generate_plugin_registry(out: &std::path::Path) {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let plugin_dir = manifest_dir.join("src/platform/ui/window/plugins");
    println!("cargo:rerun-if-changed={}", plugin_dir.display());

    let feature_dir = manifest_dir.join("../photon-core/src/features");
    println!("cargo:rerun-if-changed={}", feature_dir.display());
    let mut feature_modules = std::fs::read_dir(&feature_dir)
        .expect("read core feature modules")
        .map(|entry| entry.expect("read feature directory entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .map(|path| path.file_stem().unwrap().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    feature_modules.sort();

    let mut plugin_files = std::fs::read_dir(&plugin_dir)
        .expect("read built-in shell plugin modules")
        .map(|entry| entry.expect("read plugin directory entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .filter(|path| path.file_stem().is_some_and(|stem| stem != "mod"))
        .collect::<Vec<_>>();
    plugin_files.sort();
    assert!(
        plugin_files
            .iter()
            .any(|path| path.file_stem().is_some_and(|stem| stem == "core")),
        "the shell plugin registry requires a core plugin module"
    );
    let mut shell_features = plugin_files
        .iter()
        .filter_map(|path| {
            let module = path.file_stem()?.to_str()?;
            (module != "core").then(|| module.to_owned())
        })
        .collect::<Vec<_>>();
    shell_features.sort();
    assert_eq!(
        shell_features, feature_modules,
        "each core feature module must have a shell plugin module with the same filename"
    );

    let mut generated = String::from("// Generated from window/plugins by build.rs.\n");
    let mut modules = Vec::new();
    for path in &plugin_files {
        println!("cargo:rerun-if-changed={}", path.display());
        let module = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .expect("plugin filenames are valid UTF-8");
        assert!(
            module
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'),
            "plugin module names must use snake_case: {module}"
        );
        generated.push_str(&format!(
            "#[path = {:?}]\nmod {module};\n",
            path.display().to_string()
        ));
        modules.push(module);
    }
    generated.push_str("\nconst BUILTIN_PLUGINS: &[BuiltinPlugin] = &[\n");
    for module in modules {
        generated.push_str(&format!("    {module}::PLUGIN,\n"));
    }
    generated.push_str("];\n");

    std::fs::write(out.join("shell_plugin_registry.rs"), generated)
        .expect("write generated shell plugin registry");
}

fn generate_background_noise(out: &std::path::Path) {
    let tile = image::load_from_memory(include_bytes!("assets/zen-grain-bg.png"))
        .expect("Zen grain texture is a valid PNG")
        .to_rgba8();
    let (tile_width, tile_height) = tile.dimensions();
    let pixels =
        image::ImageBuffer::from_fn(BACKGROUND_NOISE_SIZE, BACKGROUND_NOISE_SIZE, |x, y| {
            *tile.get_pixel(x % tile_width, y % tile_height)
        });
    std::fs::write(out.join("background-noise.rgba"), pixels.into_raw())
        .expect("write tiled background noise texture");
}
