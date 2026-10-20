<p align="center"><img src="../photon-brand/assets/logo.svg" alt="Photon" width="72" /></p>

# `photon-storage`

File-backed persistence for a Photon profile: browser settings, history, sessions, favicons, site data, and the Engine cache. It uses plain Rust file access and does not depend on GPUI or Engine types.

## Use

Open a profile directory and load its saved history:

```rust
use photon_storage::Profile;

let profile = Profile::open(profile_directory)?;
let history = profile.load_history();
```

`Profile::open_default()` selects the platform’s default location, or the path in `PHOTON_PROFILE_DIR`. Data models and browser rules come from [`photon-core`](../photon-core/README.md).
