#!/bin/bash
set -euo pipefail

repo_root="$(cd "${PROJECT_DIR}/.." && pwd)"
output_dir="${DERIVED_FILE_DIR}"
configuration="${CONFIGURATION:-Debug}"
sdk_name="${SDK_NAME:-}"
architectures="${ARCHS:-arm64}"

if ! command -v cargo >/dev/null 2>&1; then
    echo "error: cargo was not found in PATH. Install Rust and run photon ios build from a terminal." >&2
    exit 1
fi

if ! command -v rustup >/dev/null 2>&1; then
    echo "error: rustup was not found in PATH. Install Rust with rustup before building the iOS target." >&2
    exit 1
fi

if [[ "${configuration}" == "Release" ]]; then
    cargo_profile="release"
    cargo_profile_args=(--release)
else
    cargo_profile="debug"
    cargo_profile_args=()
fi

libraries=()
for architecture in ${architectures}; do
    if [[ "${sdk_name}" == iphoneos* ]]; then
        case "${architecture}" in
            arm64) rust_target="aarch64-apple-ios" ;;
            *) echo "error: unsupported iPhoneOS architecture: ${architecture}" >&2; exit 1 ;;
        esac
    elif [[ "${sdk_name}" == iphonesimulator* ]]; then
        case "${architecture}" in
            arm64) rust_target="aarch64-apple-ios-sim" ;;
            x86_64) rust_target="x86_64-apple-ios" ;;
            *) echo "error: unsupported iOS Simulator architecture: ${architecture}" >&2; exit 1 ;;
        esac
    else
        echo "error: expected an iOS SDK, got ${sdk_name}" >&2
        exit 1
    fi

    if ! rustup target list --installed | grep -Fxq "${rust_target}"; then
        rustup target add "${rust_target}"
    fi

    cargo build \
        --manifest-path "${repo_root}/Cargo.toml" \
        --package photon-ffi \
        --target "${rust_target}" \
        "${cargo_profile_args[@]}"
    libraries+=("${repo_root}/target/${rust_target}/${cargo_profile}/libphoton_ffi.a")
done

mkdir -p "${output_dir}"
if ((${#libraries[@]} == 1)); then
    cp "${libraries[0]}" "${output_dir}/libphoton_ffi.a"
else
    xcrun lipo -create "${libraries[@]}" -output "${output_dir}/libphoton_ffi.a"
fi
