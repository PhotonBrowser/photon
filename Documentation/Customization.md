# Customizable browser workspace

**Status: proposal.** This document describes the architecture and delivery
sequence for making Photon's browser chrome configurable. The first milestone
is a behavior-preserving refactor of the existing layouts. The edit experience
comes after that foundation.

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

## Architecture direction

### Separate the layout model from its renderers

The saved layout model should contain only stable, serializable browser data:
component identifiers, visibility, dock edge, order, and optional size
preferences. It must not contain GPUI entities, closures, native handles, or
Engine types.

`photon-core` can own the framework-independent layout and preset types as part
of browser settings. `photon-storage` can persist them through the existing
profile settings file. The shell owns component definitions, GPUI renderers,
drag interactions, the dock host, and the adapter from a saved layout to the
active window. Keep the model independent of UI frameworks and keep each
component's internal browser behavior with the component that owns it.

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

Only after both built-in layouts use the new host should the profile settings
grow named layout presets and per-space layouts. Keep a profile-level default
for new spaces. In the first version, applying a preset to a space copies its
layout; later edits to that space do not silently mutate the reusable preset
or other spaces. Users can save a customized layout as a new preset.

The active space is currently profile-wide, so its layout will also be
consistent across windows under the existing behavior. Per-window active
spaces would require a separate state-model decision and are outside this
proposal.

### Keep the editor reusable and recoverable

Edit mode should spread the current components apart slightly, show labels
and grab handles, and expose drop regions. A component tray should let people
add hidden components, and each dock should support reordering. The page stays
in the center. Show a live preview with Done, Cancel, Undo, and Reset so people
can experiment without losing their previous layout.

The editor should operate on a draft layout and commit it through one settings
update. Its reusable parts are the layout draft, validation and constraint
rules, component catalog, and editor interactions. Onboarding may reuse the
catalog and preset selection without being forced to use the full drag editor.

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
3. **Add the saved model.** Add serializable presets and per-space layout
   configuration with defaults for old profiles and new spaces. Keep the
   active-space scope consistent with current behavior.
4. **Build edit mode.** Add the component catalog, four dock regions,
   add/remove/reorder/move interactions, bounded resizing, accessible
   alternatives to dragging, and draft/commit/revert behavior.
5. **Grow the catalog and reuse it.** Add components as browser features
   become available, keep true feature switches separate from visibility,
   and use presets and component metadata in onboarding.

Do not start step 4 by adding layout-specific branches throughout existing
views. The composition refactor in step 2 is the prerequisite for a clean,
maintainable editor.

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
- Onboarding can recommend or apply a preset by using the same layout and
  settings model as normal browser customization.
- Layout configuration remains in Rust; page rendering and the Engine
  integration are unaffected.
