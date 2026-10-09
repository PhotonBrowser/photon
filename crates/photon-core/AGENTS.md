# photon-core

Keep this crate framework-independent. It owns browser state, diagnostics,
commands, and shared address normalization. Do not add GPUI, platform handles,
raw C pointers, or Ladybird types here. The exported C ABI belongs in
`photon-ffi`.
