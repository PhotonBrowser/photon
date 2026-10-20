<p align="center"><img src="../photon-brand/assets/logo.svg" alt="Photon" width="72" /></p>

# `photon-shortcuts`

The shared registry of Photon browser actions and default GPUI-CE key bindings. Menus and shortcut listings use the same catalog so labels and actions stay aligned.

## Use

`browser_shortcuts()` returns the browser’s default `Vec<KeyBinding>` for installation by the shell. The catalog helpers expose grouped shortcut entries and map [`photon-core`](../photon-core/README.md) commands to their actions.

This is an internal Photon workspace crate because its action types and bindings are tied to Photon’s browser commands and GPUI-CE.
