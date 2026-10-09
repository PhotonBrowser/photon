# photon-ffi

This crate is the Rust-to-C adapter. Keep exported symbols, raw pointer
validation, null-terminated string handling, and ABI-specific state here. Keep
`include/photon_ffi.h` in sync with function signatures and field values.
Delegate browser rules to `photon-core`. Do not add UI framework or Engine
types to this crate. Document safety requirements on every exported unsafe
function.
