# Plugins and optional features

Photon treats each optional browser capability as a built-in plugin. Every
plugin has its own manifest module in `crates/photon-core/src/features/`. The
core build script discovers those files and generates the registry and
`Feature` constants, so adding a feature module adds its stable ID and settings
entry without editing a central catalog. The module declares the feature's
name, description, settings category, preference, and dependencies.

Users can enable or disable plugins under **Settings → Advanced → Features**.
The registry covers navigation, page tools, appearance, interface effects,
new-tab controls, and tab management, including Liquid Glass and background
texture.

To add a built-in plugin, create a snake-case Rust module in
`crates/photon-core/src/features/` and define its `DEFINITION` with the
`feature!` macro. The filename supplies its stable kebab-case key and generated
`Feature::Name` constant. Add a page or workspace component owner only when the
plugin contributes that surface; add a window cleanup hook only when disabling
it must dismiss active transient UI.

The shell connects plugin IDs to the surfaces they own:

- Internal pages declare an owning feature in `PageDefinition`.
- Workspace components declare an owning feature in the component catalog.
- Window plugins can register cleanup hooks for transient UI and state.

When a feature is disabled, its page and controls stop rendering, open
feature-owned transient UI is dismissed, and disabled workspace components are
removed from a resolved copy of the saved layout. The saved layout remains
unchanged, so re-enabling a plugin restores the user's placements. Empty docks
are omitted and their extents are recalculated from the remaining components.

Disabling a plugin also stops its active work where it owns any: tab sleeping
drops its scheduler and OS memory-pressure monitor, tab archiving drops its
periodic timer, Downloads drops its window redraw observer, and performance
pages drop their samplers and monitoring leases. Glass and background-noise
caches are released when those features are turned off. The built-in Rust code
is still statically linked into Photon; runtime switches release resources and
stop work, but do not unload machine code from the process.

## Third-party extensions

Third-party extensions are not loadable yet. The current registries are
build-generated from built-in Rust feature modules, and extension code cannot
be added by a user.
The built-in boundaries provide a starting point for exposing supported
extension points without giving extensions access to arbitrary browser or
engine internals.

Before third-party extensions are introduced, define a versioned manifest and
namespaced plugin IDs, a stable host API for pages, commands, and workspace
components, and explicit per-plugin permissions. Extension execution should
be isolated from the browser shell, with user-controlled installation,
enablement, removal, and data storage. Extensions must not load arbitrary Rust
dynamic libraries into the browser process or access Ladybird internals.
