# Workspace presets

Photon uses named workspace presets to choose horizontal or vertical tabs and
the side of the window used by the sidebar. In Settings → Appearance, people
can view, create, edit and delete presets, choose the profile default, and
assign a preset to the active space. The New Space and Edit Space dialogs also
let people choose a preset or create one for that space. Each space keeps its
own window colour and gradient.

## Behavior

- Horizontal and Vertical presets are available by default and can be edited or
  deleted like other presets.
- Each preset has a name, tab orientation, and sidebar side.
- At least one preset is kept so every profile and space has a valid layout.
- Spaces follow the profile default unless they select a named preset.
- A space's preset is stored by ID, so it follows later changes to that preset.
- If a preset is deleted, spaces assigned to it follow the profile default.
- Legacy per-space layout snapshots continue to load until the user selects a
  named preset or the profile default.
- Window colour and gradient remain independent per-space appearance settings.

## Ownership

`photon-core/src/workspace.rs` owns the serializable preset model and layout
values. `photon-core/src/spaces.rs` owns each space's preset assignment and
legacy layout snapshot. `photon-storage` persists those settings in the
existing profile document.

The shell's `ui/workspace_layout.rs` resolves saved presets through the browser
chrome catalog. `ui/window/chrome.rs` and `ui/window/workspace.rs` render the
selected composition. Settings → Appearance manages presets, selects the
profile default, and assigns a preset to the active space. The space dialog can
create and assign a preset while creating or editing a space.

The model and preset selection can support first-run onboarding later without
making the browser window responsible for onboarding flow. The shell continues
to own GPUI rendering and browser interactions; layout data remains independent
of GPUI, native handles, and Engine types.
