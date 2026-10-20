<p align="center"><img src="assets/logo.svg" alt="Photon" width="72" /></p>

# `photon-brand`

The single source for Photon’s browser name, engine name, internal page scheme, profile folder, app icon, and logo. Other crates use these values instead of repeating brand strings.

## Use

```rust
use photon_brand::{ENGINE_NAME, LOGO_SVG, NAME, PAGE_SCHEME};

assert_eq!(NAME, "Photon");
assert_eq!(ENGINE_NAME, "Photon Engine");
let settings_url = format!("{PAGE_SCHEME}://settings");
let logo = LOGO_SVG;
```

The crate has no dependencies so every Photon crate can use it. Update the assets and constants here when changing the browser identity.
