<p align="center"><img src="../photon-brand/assets/logo.svg" alt="Photon" width="72" /></p>

# `photon-ffi`

Photon’s Rust-to-C adapter. It exports the small browser state and navigation API used by native embedder code; browser rules remain in [`photon-core`](../photon-core/README.md).

## Use from C

Include [`photon_ffi.h`](include/photon_ffi.h), create a handle, and destroy it when finished:

```c
#include "photon_ffi.h"

PhotonBrowserHandle *browser = photon_browser_create();
char destination[4096];
if (photon_browser_navigate(browser, "example.com", destination,
        sizeof(destination))) {
    /* destination now contains a null-terminated URL */
}
photon_browser_destroy(browser);
```

The build produces Rust and static-library targets. Strings returned by the API are borrowed; their lifetime and mutation rules are documented in the header.
