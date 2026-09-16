# AGENTS.md

## Project

Photon is a Chromium-based browser fork built on top of Helium.

The intended architecture is:

Chromium
→ Helium patches
→ Photon overlay patches

Photon should preserve Helium internals where practical and apply focused Photon-specific changes on top.

## Core rules

- Do not globally replace `Helium` with `Photon`.
- Do not globally replace `helium` with `photon`.
- Do not modify `patches/helium/**` unless there is no reasonable overlay alternative.
- Prefer new Photon patches under:
  - `helium-chromium/patches/photon/`
  - `patches/photon/`
- Keep Photon patches focused and small.
- Preserve upstream attribution and license notices.
- Do not submit Photon-specific patches upstream to `imputnet/*`.

## Branding

User-visible product branding should generally say `Photon`.

Examples:
- `About Helium` → `About Photon`
- `Customize Helium` → `Customize Photon`
- visible product name `Helium` → `Photon`
- visible product metadata may use `The Photon Authors`

Internal identifiers should generally remain Helium.

Examples to preserve:
- `kHelium*`
- `HELIUM_*`
- `GetHeliumVersionNumber()`
- `helium.browser.*`
- internal Helium namespaces/classes/functions
- `helium://` internal routes unless there is a specific reason to rename them
- executable/profile/data-dir identifiers
- update/signing/service internals

## Copyright and attribution

Photon may use `The Photon Authors` for Photon-owned product metadata and Photon-authored code.

Do not erase upstream attribution.

Preserve:
- Helium source copyright notices
- Chromium notices
- third-party notices
- LICENSE/CREDITS attribution
- vendor attribution

For substantially derived files, preserve upstream notices and add Photon attribution if appropriate.

## Patch workflow

Prefer:

1. inspect the relevant Helium patch
2. identify the user-visible behavior/string to override
3. create a focused Photon overlay patch
4. place it after the Helium patch it depends on
5. verify `patches/helium/**` remains unchanged

Do not create giant duplicated copies of Helium patches unless absolutely necessary.

## Build safety

This machine has limited resources.

Never run destructive/full rebuild commands unless explicitly requested.

Do not:
- delete `build/src`
- delete `build/src/out/Default`
- run clean/clobber commands
- run the full Docker build for normal development

Use incremental builds against the existing output directory.

Preferred safe build:

```bash
source scripts/dev.sh
he build
