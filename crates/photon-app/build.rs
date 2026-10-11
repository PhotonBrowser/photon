//! Lets the browser find the Engine's libraries when macOS launches it from
//! its app bundle, without `photon run`'s library path.

use std::{env, path::PathBuf};

fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let engine_build = env::var_os("PHOTON_ENGINE_BUILD_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("build/engine-release"));
    for libraries in ["lib64", "lib"] {
        let path = engine_build.join(libraries);
        if let Ok(path) = path.canonicalize() {
            println!("cargo:rustc-link-arg-bins=-Wl,-rpath,{}", path.display());
        }
    }
    println!("cargo:rerun-if-env-changed=PHOTON_ENGINE_BUILD_DIR");
}
