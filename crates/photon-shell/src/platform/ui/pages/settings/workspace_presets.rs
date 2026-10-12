//! Creating, editing, deleting and confirming workspace presets.

use gpui::{AnyElement, Context, Focusable, KeyDownEvent, Role, Window, prelude::*, px, rgb};
use gpui_elements::editable_text::{EditableTextState, StringStorage, text_input};
use photon_core::{
    SidebarSide, TabLayout, WorkspaceLayout as SavedWorkspaceLayout, WorkspacePreset,
};

use super::super::super::button::{ButtonKind, ButtonSize, button};
use super::super::super::controls::text_field;
use super::super::super::layout::{error_text, form_field, h_stack};
use super::super::super::modal::{MODAL_MOTION, dialog_title, modal, modal_panel};
use super::super::super::settings::Settings;
use super::super::super::{metrics, theme::ThemeColors};
use super::super::layout::secondary_text;
use super::{SettingsPage, WorkspacePresetDraft};

impl SettingsPage {
    pub(super) fn start_workspace_preset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_workspace_preset(None, window, cx);
    }

    pub(super) fn edit_workspace_preset(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(preset) = Settings::get(cx)
            .workspace
            .presets
            .iter()
            .find(|preset| preset.id == id)
            .cloned()
        else {
            return;
        };
        self.open_workspace_preset(Some(preset), window, cx);
    }

    pub(super) fn ask_to_delete_workspace_preset(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let preset = Settings::get(cx)
            .workspace
            .presets
            .iter()
            .find(|preset| preset.id == id)
            .cloned();
        if let Some(preset) = preset {
            self.pending_delete_preset = Some(preset);
            window.focus(&self.delete_preset_focus, cx);
            cx.notify();
        }
    }

    fn open_workspace_preset(
        &mut self,
        preset: Option<WorkspacePreset>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = cx.new(|cx| EditableTextState::new(StringStorage::default(), cx));
        if let Some(preset) = preset.as_ref() {
            name.update(cx, |name, cx| name.emplace(&preset.name, cx));
        }
        let name_observer = cx.observe(&name, |_, _, cx| cx.notify());
        let saved_layout = preset.as_ref().map_or_else(
            || {
                super::super::super::workspace_layout::WorkspaceLayout::current(cx)
                    .saved()
                    .clone()
            },
            |preset| preset.layout.clone(),
        );
        window.focus(&name.focus_handle(cx), cx);
        self.workspace_preset_draft = Some(WorkspacePresetDraft {
            preset_id: preset.map(|preset| preset.id),
            name,
            tab_layout: saved_layout.tab_layout(),
            sidebar_side: saved_layout.sidebar_side(),
            invalid: false,
            _name_observer: name_observer,
        });
        cx.notify();
    }

    fn set_preset_tab_layout(&mut self, tab_layout: TabLayout, cx: &mut Context<Self>) {
        if let Some(draft) = self.workspace_preset_draft.as_mut() {
            draft.tab_layout = tab_layout;
            draft.invalid = false;
            cx.notify();
        }
    }

    fn set_preset_sidebar_side(&mut self, sidebar_side: SidebarSide, cx: &mut Context<Self>) {
        if let Some(draft) = self.workspace_preset_draft.as_mut() {
            draft.sidebar_side = sidebar_side;
            draft.invalid = false;
            cx.notify();
        }
    }

    fn save_workspace_preset(&mut self, cx: &mut Context<Self>) {
        let Some(draft) = self.workspace_preset_draft.as_ref() else {
            return;
        };
        let name = draft.name.read(cx).as_str().trim().to_owned();
        let tab_layout = draft.tab_layout;
        let preset_id = draft.preset_id.clone();
        let layout = SavedWorkspaceLayout::for_tab_layout_and_side(tab_layout, draft.sidebar_side);
        let mut workspace = Settings::get(cx).workspace.clone();
        let saved = match preset_id {
            Some(id) => workspace.update_preset(&id, &name, layout),
            None => workspace.add_preset(&name, layout).is_some(),
        };
        if !saved {
            if let Some(draft) = self.workspace_preset_draft.as_mut() {
                draft.invalid = true;
            }
            cx.notify();
            return;
        }

        Settings::update(cx, move |settings| {
            settings.workspace = workspace;
            settings.tab_layout = settings
                .workspace
                .default_preset()
                .map_or(tab_layout, |preset| preset.layout.tab_layout());
        });
        self.workspace_preset_draft = None;
        cx.notify();
    }

    fn cancel_workspace_preset(&mut self, cx: &mut Context<Self>) {
        self.workspace_preset_draft = None;
        cx.notify();
    }

    fn confirm_delete_workspace_preset(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self
            .pending_delete_preset
            .as_ref()
            .map(|preset| preset.id.clone())
        else {
            return;
        };
        Settings::update(cx, move |settings| {
            if settings.workspace.remove_preset(&id) {
                settings.spaces.remove_preset_override(&id);
                if let Some(default) = settings.workspace.default_preset() {
                    settings.tab_layout = default.layout.tab_layout();
                }
            }
        });
        self.pending_delete_preset = None;
        cx.notify();
    }

    fn cancel_delete_workspace_preset(&mut self, cx: &mut Context<Self>) {
        self.pending_delete_preset = None;
        cx.notify();
    }

    pub(super) fn workspace_preset_dialog(
        &mut self,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let animations = Settings::get(cx).animations;
        let ((), transition) = self.workspace_preset_presence.sync(
            self.workspace_preset_draft.as_ref().map(|_| ()),
            MODAL_MOTION,
            &animations,
            cx,
        )?;
        let draft = self.workspace_preset_draft.as_ref()?;
        let editing = draft.preset_id.is_some();
        let title = if editing {
            "Edit layout preset"
        } else {
            "New layout preset"
        };
        let name_field = form_field(
            "Name",
            text_field(
                text_input("settings-preset-name")
                    .state(draft.name.downgrade())
                    .track_focus(&draft.name.focus_handle(cx))
                    .placeholder("Preset name")
                    .placeholder_color(rgb(palette.text_secondary)),
                palette,
            ),
            palette,
        );
        let layout_fields = super::super::super::workspace_preset_fields::fields(
            "settings-preset-tabs",
            "settings-preset-sidebar",
            draft.tab_layout,
            draft.sidebar_side,
            palette,
            |layout| {
                Box::new(cx.listener(move |this, _, _, cx| this.set_preset_tab_layout(layout, cx)))
            },
            |side| {
                Box::new(cx.listener(move |this, _, _, cx| this.set_preset_sidebar_side(side, cx)))
            },
        );
        let mut panel = modal_panel(palette)
            .id("settings-layout-preset-dialog")
            .role(Role::Dialog)
            .aria_label(title)
            .aria_modal(true)
            .track_focus(&draft.name.focus_handle(cx))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.modifiers.modified() {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "enter" => this.save_workspace_preset(cx),
                    "escape" => this.cancel_workspace_preset(cx),
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .child(dialog_title(title, palette))
            .child(name_field)
            .child(layout_fields);
        if draft.invalid {
            panel = panel.child(error_text(
                if editing {
                    "Enter a name of up to 64 characters."
                } else {
                    "Enter a name of up to 64 characters. The profile can have up to 32 presets."
                },
                palette,
            ));
        }
        panel = panel.child(
            h_stack()
                .justify_end()
                .gap(px(metrics::BUTTON_GAP))
                .child(button(
                    "settings-preset-cancel",
                    "Cancel",
                    ButtonKind::Secondary,
                    true,
                    ButtonSize::Regular,
                    palette,
                    Box::new(cx.listener(|this, _, _, cx| this.cancel_workspace_preset(cx))),
                ))
                .child(button(
                    "settings-preset-save",
                    if editing {
                        "Save changes"
                    } else {
                        "Create preset"
                    },
                    ButtonKind::Primary,
                    true,
                    ButtonSize::Regular,
                    palette,
                    Box::new(cx.listener(|this, _, _, cx| this.save_workspace_preset(cx))),
                )),
        );
        Some(
            modal(
                "settings-create-preset-modal",
                transition,
                palette,
                panel,
                cx,
            )
            .into_any_element(),
        )
    }

    pub(super) fn workspace_preset_delete_dialog(
        &mut self,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let animations = Settings::get(cx).animations;
        let ((), transition) = self.delete_preset_presence.sync(
            self.pending_delete_preset.as_ref().map(|_| ()),
            MODAL_MOTION,
            &animations,
            cx,
        )?;
        let preset = self.pending_delete_preset.as_ref()?;
        let title = format!("Delete ‘{}’?", preset.name);
        let message =
            "Spaces using this preset will follow the profile default. This cannot be undone.";
        let panel = modal_panel(palette)
            .id("settings-delete-preset-dialog")
            .role(Role::AlertDialog)
            .aria_label("Delete layout preset")
            .aria_modal(true)
            .track_focus(&self.delete_preset_focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.modifiers.modified() {
                    return;
                }
                // Enter does not delete: only the button does.
                if event.keystroke.key == "escape" {
                    this.cancel_delete_workspace_preset(cx);
                    cx.stop_propagation();
                }
            }))
            .child(dialog_title(title, palette))
            .child(secondary_text(message, palette))
            .child(
                h_stack()
                    .justify_end()
                    .gap(px(metrics::BUTTON_GAP))
                    .child(button(
                        "settings-delete-preset-cancel",
                        "Cancel",
                        ButtonKind::Secondary,
                        true,
                        ButtonSize::Regular,
                        palette,
                        Box::new(
                            cx.listener(|this, _, _, cx| this.cancel_delete_workspace_preset(cx)),
                        ),
                    ))
                    .child(button(
                        "settings-delete-preset-confirm",
                        "Delete preset",
                        ButtonKind::Destructive,
                        true,
                        ButtonSize::Regular,
                        palette,
                        Box::new(
                            cx.listener(|this, _, _, cx| this.confirm_delete_workspace_preset(cx)),
                        ),
                    )),
            );
        Some(
            modal(
                "settings-delete-preset-modal",
                transition,
                palette,
                panel,
                cx,
            )
            .into_any_element(),
        )
    }
}
