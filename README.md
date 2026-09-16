<div align="center">
    <img src="resources/branding/app_icon/raw.png"
        title="Photon" alt="Photon logo" width="120" />
    <h1>Photon Chromium Layer</h1>
    <p>
        Photon is a user-facing browser fork built on top of Helium and Chromium.
        <br>
        This repository contains the shared Chromium patches and resources.
    </p>
</div>

## Project relationship

Photon keeps the upstream Helium implementation and Chromium base, then adds
focused Photon overlays for the visible product experience. Helium internals,
including compatibility identifiers and upstream patch structure, are retained
where changing them would break the build or integrations.

## Development

The Linux development checkout is the sibling repository at
`photon-linux/`. From that repository’s root:

```bash
ph setup
ph build
ph run
```

`ph run` checks for source changes and asks whether to build before launching.
The existing `he` function remains available for compatibility, but `ph` is the
recommended command.

## Patch layers

Patch ownership is explicit:

- `patches/helium/` contains imported Helium patches and should remain untouched
  for Photon-only work.
- `patches/photon/` contains Photon overlays and is the normal place for
  Photon-specific changes.
- `patches/series` controls application order. Put a Photon overlay after the
  Helium or Chromium patch whose behavior it changes.

To create a Photon patch, load the Linux development environment, enter the
applied source tree, create a `photon/` patch with quilt, edit the source, and
refresh it:

```bash
cd ../
source scripts/dev.sh
cd build/src
quilt new photon/my-change.patch
quilt add path/to/file
# edit the file
quilt refresh
```

Review the patch, add it to `patches/series`, and verify that it does not rename
internal Helium symbols or remove upstream attribution.

## Upstream projects

Photon is based on these upstream projects:

- [Helium](https://github.com/imputnet/helium)
- [Helium for Linux](https://github.com/imputnet/helium-linux)
- [Chromium](https://www.chromium.org/)
- [ungoogled-chromium](https://github.com/ungoogled-software/ungoogled-chromium)

The upstream Helium repositories also document the shared patch conventions
that Photon builds upon.

## Attribution and license

Photon is made possible by Chromium, Helium, and other open source software.
Do not remove Helium or Chromium copyright headers, license files, credits,
third-party notices, or upstream patch attribution. Photon-specific content is
licensed as described in [LICENSE](LICENSE); imported content retains its
original license.
