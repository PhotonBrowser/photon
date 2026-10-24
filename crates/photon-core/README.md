<p align="center"><img src="../photon-brand/assets/logo.svg" alt="Photon" width="72" /></p>

# `photon-core`

Framework-independent browser state and rules: navigation commands, settings, tabs, history, spaces, workspace layouts, internal pages, browsing-data requests, address/search resolution, suggestions, and tab-sleep policy. It contains no GPUI, native window handles, or Ladybird types.

## Use

Resolve address-bar input with the same rules used by Photon:

```rust
use photon_core::normalize_url;

assert_eq!(normalize_url("example.com").unwrap(), "https://example.com/");
```

Time and persistence are supplied by callers. See [`photon-storage`](../photon-storage/README.md) for profile files.
