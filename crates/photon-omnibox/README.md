<p align="center"><img src="../photon-brand/assets/logo.svg" alt="Photon" width="72" /></p>

# `photon-omnibox`

Framework-independent address and search rules for browser omniboxes. It classifies typed input, resolves URLs, and matches suggestions from visited pages and past searches.

## Use

```rust
use photon_omnibox::resolve;

let target = resolve("example.com").unwrap();
assert_eq!(target.url(), "https://example.com/");
```

Use [`resolve_with`](src/resolve.rs) to provide a custom search-engine registry. The browser state crate re-exports the default resolver from [`photon-core`](../photon-core/README.md).
