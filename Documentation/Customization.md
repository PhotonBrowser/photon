# Customizable browser workspace

**Status: profile presets, per-space layouts, and a full-window workspace
editor are implemented.** Its catalog now exposes the current toolbar controls
and sidebar panels individually. Custom named presets, independent feature
switches, and onboarding integration remain planned.

## Goal

People should be able to choose which browser features are available, choose
which controls and panels are visible, arrange those components around the
page, and save useful arrangements for different spaces. The same component
catalog and presets should support future first-run onboarding and guided
setup.

Customization belongs to Photon's native Rust and GPUI shell. It does not
change how pages are rendered, expose Engine internals, or put browser UI in a
web view.

## Terms and behavior

Keep feature availability separate from the placement of its UI:

| Term | Meaning | Example |
| --- | --- | --- |
| Feature | A browser capability that can be enabled or disabled | Downloads, history |
| Component | A shell surface or control that presents browser functionality | Tab list, downloads panel, address field |
| Layout | The visibility, dock edge, order, and size of components | Tabs on the left; tools on the right |
| Preset | A named, reusable layout template | Standard, Minimal, Research |
| Space layout | The layout currently used by a space | A Research layout for a Study space |

Hiding a component only changes the shell. Its feature remains available from
other supported entry points, such as the browser menu or command bar. A
feature switch controls the capability and its entry points. The UI should
make these two actions clear and should preserve any browser data according to
the feature's own retention rules.

## Current behavior to preserve

Today, Appearance settings choose between a horizontal tab strip and a vertical
sidebar. The vertical sidebar contains navigation, the address field,
favourites, pinned and current tabs, and a footer. It has its own reveal,
resize, drag, keyboard, and space-switching behavior. Spaces currently hold a
name, color, gradient setting, and pinned tabs. The active space is shared
across windows.

The new composition layer must initially reproduce both existing layouts and
their behavior. Existing profiles should keep their current layout choice;
missing layout data should resolve to the current default. Sidebar reveal and
resize, tab ordering and menus, keyboard shortcuts, favourites, space
switching, theme, and page sizing must continue to work as they do now.

The saved model lives in `photon-core/src/workspace.rs`; profile storage
migrates older `TabLayout` settings into the matching built-in preset. Each
`Space` may store a copied layout override, while spaces without an override
follow the profile default. The shell catalog and resolver live in
`ui/workspace_layout.rs`; `ui/window/chrome.rs` renders the chosen components,
and `ui/window/workspace.rs` composes them around the page. Appearance settings
can change the profile default or assign a built-in layout to the active
space. `ui/window/workspace_edit.rs` owns the draft editor and commits its
result to the edited space.

## Architecture direction

### Separate the layout model from its renderers

The saved layout model should contain only stable, serializable browser data:
component identifiers, visibility, dock edge, order, and optional size
preferences. It must not contain GPUI entities, closures, native handles, or
Engine types.

`photon-core` owns the framework-independent layout and preset types as part
of browser settings. `photon-storage` persists them through the existing
profile settings file and migrates older `TabLayout` settings. The shell owns
component definitions, GPUI renderers, drag interactions, the dock host, and
the adapter from a saved layout to the active window. Keep the model
independent of UI frameworks and keep each component's internal browser
behavior with the component that owns it.

The layout host should expose four dock regions around a central page area:
top, bottom, leading side, and trailing side. Components may stack and be
reordered in a region. The host enforces supported placements and size limits
so browser chrome cannot consume the whole page. Leading and trailing are
logical directions, so the layout remains suitable for right-to-left
interfaces.

Use a small shell-owned component catalog to describe each component: stable
ID, user-facing name and group, default placement, allowed edges, visibility
and sizing constraints, and whether the component is available in the current
configuration. Keep the catalog descriptive. Feature behavior and rendering
remain owned by their existing Rust modules; this proposal does not call for a
general plugin system.

The editor should consume the catalog and saved layout model rather than know
how each feature works. That seam lets future setup flows reuse component
descriptions and presets without embedding onboarding logic in every browser
view. Onboarding can recommend or apply a preset, then hand off to the same
settings model used by the editor.

### Preserve defaults through an adapter

First extract the current horizontal and vertical composition behind the new
layout host. Map the existing `TabLayout` setting to those built-in layouts and
keep its current persisted representation readable during the transition.
Do not introduce edit mode, new default placements, or feature switches in
this refactor. The visible result should remain the same while the shell gains
a stable boundary for future layouts.

Both built-in layouts now resolve through the profile's named preset model or
the active space's copied layout. The storage adapter migrates the legacy
`TabLayout` field into the matching built-in preset while keeping that field
readable during the transition. Spaces without an override follow the profile
default; Appearance settings can assign a copied built-in layout or restore
that default. The editor saves a per-space copy, and resetting to the profile
default returns that space to following the default. Later edits to a space do
not mutate the reusable preset or other spaces.

The active space is currently profile-wide, so its layout will also be
consistent across windows under the existing behavior. Per-window active
spaces would require a separate state-model decision and are outside this
proposal.

### Keep the editor reusable and recoverable

**Edit Workspace** transforms the current browser window in place. The live
page stays visible inside its padded area while browser chrome is disabled for
interaction. Components spread into outlined frames across four docks, and a
component library stays at the bottom. Drag a frame's grip to move or reorder
it. Drop a visible component into the library to remove it; drag a library item
into a dock to add it. Valid library drops get a visible highlight.

The catalog describes the tab strip, Back, Forward, Reload, address field,
browser menu, sidebar toggle, favourites panel, vertical tabs, and sidebar
footer as separate entries. Each entry supplies a stable ID, description,
default edge, supported edges, capability groups, and optional size bounds.
The side dock keeps its visible panels on one edge, and horizontal and vertical
tabs cannot be active together. Existing saved `address-toolbar` and `sidebar`
entries expand through a shell adapter, so their current horizontal or vertical
layout still resolves to the same controls. The catalog is the extension
point for future controls and panels; adding a new feature still belongs in
its existing Rust module and renderer. The canvas edits a draft, and the live
browser chrome changes after **Done** commits it.

The editor operates on a draft layout with **Undo**, **Reset**, **Cancel**, and
**Done**. Reset restores the profile's default layout for the current space;
Undo can restore the previous draft, Cancel discards edits, and Done saves the
draft. Every browser chrome component can be removed, leaving the padded page
area. The reusable parts are the framework-independent layout model, shell
component catalog, group validation rules, and dock metadata. The first library
uses current shell pieces; it does not add new feature panels, custom named
presets, or independent capability switches. Onboarding can reuse the catalog
and preset model without using the full drag editor.

Every drag operation needs an equivalent keyboard route. Hidden components
need a visible discovery path; customization must not strand users without a
way to open settings or restore a known layout. Motion should follow the
existing reduced-motion behavior. New colors and dimensions belong in the
shared theme and metrics modules.

## Delivery sequence

1. **Inventory and baseline.** Record the current horizontal and vertical
   layouts, including keyboard, pointer, resize, reveal, menu, theme, and
   multiple-window behavior. Decide the stable component IDs and layout
   boundaries before changing composition code.
2. **Extract the composition layer.** Split the fixed window composition into
   reusable shell layout and component modules. Route the existing two
   layouts through the new host without changing persisted settings or user
   behavior.
3. **Add the profile model.** Add serializable layouts and named profile
   presets. Migrate the existing `TabLayout` setting without changing the
   layout current profiles use. **Complete.**
4. **Add per-space layouts.** Let each space use a copied preset layout, with
   the profile default for spaces without an override. Keep active-space scope
   consistent with current behavior. **Complete.**
5. **Build edit mode.** Add the component catalog, four dock regions,
   add/remove/reorder/move interactions, bounded resizing, accessible
   alternatives to dragging, and draft/commit/revert behavior. **Complete.**
6. **Grow the catalog and reuse it.** Add components as browser features
   become available, keep true feature switches separate from visibility,
   and use presets and component metadata in onboarding.

Keep layout-specific composition in the workspace adapter instead of adding
branches throughout feature views. The composition refactor is the
prerequisite for a clean, maintainable editor.

## Completion criteria

- Existing horizontal and vertical layouts remain the defaults users see,
  with existing profile settings and interactions intact.
- Component placement is driven by saved data and the shell catalog, not by
  hard-coded layout branches in each feature view.
- A profile default, per-space layouts, and named presets have explicit,
  predictable save and copy behavior.
- A hidden component can be found and restored, and users can always reach a
  recovery path.
- The editor is keyboard operable, reports component names and states, and
  respects reduced motion.
- Custom named presets, independent feature switches, and onboarding can use
  the same layout and settings model as normal browser customization.
- Layout configuration remains in Rust; page rendering and the Engine
  integration are unaffected.
