//! The dialog for creating a space or changing one: its name and its window
//! colour. The window shows it as a modal and carries out the result.

use gpui::{
    Context, Entity, EventEmitter, FocusHandle, Focusable, KeyDownEvent, Render, Role,
    Subscription, Window, div, prelude::*, px, rgb,
};
use gpui_elements::editable_text::{EditableTextState, StringStorage, text_input};
use photon_core::{SidebarSide, Space, TabLayout, WindowColor, WorkspaceLayout};
use std::rc::Rc;

use super::button::{ButtonKind, ButtonSize, button};
use super::color_picker::color_picker;
use super::controls::{Choice, text_field};
use super::dropdown::dropdown;
use super::layout::{error_text, form_field, h_stack, v_stack};
use super::modal::{dialog_title, modal_panel};
use super::settings::Settings;
use super::{metrics, theme::palette};

/// What the dialog asks its window to do. Each closes it.
pub(super) enum SpaceEditorEvent {
    Save {
        name: String,
        color: WindowColor,
        gradient: bool,
        preset_id: Option<String>,
        new_preset: Option<(String, WorkspaceLayout)>,
    },
    Cancel,
}

impl EventEmitter<SpaceEditorEvent> for SpaceEditor {}

pub(super) struct SpaceEditor {
    /// Whether it creates a space rather than changing one.
    creating: bool,
    name: Entity<EditableTextState>,
    color: WindowColor,
    gradient: bool,
    preset_id: Option<String>,
    creating_preset: bool,
    preset_name: Entity<EditableTextState>,
    preset_tab_layout: TabLayout,
    preset_sidebar_side: SidebarSide,
    invalid_preset: bool,
    _name_observer: Subscription,
    _preset_name_observer: Subscription,
}

impl SpaceEditor {
    /// A dialog for changing `space`, or for a new space when `None`.
    pub(super) fn new(space: Option<&Space>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings = Settings::get(cx);
        let preset_id = space.and_then(|space| {
            space.preset_override_id.clone().or_else(|| {
                space.layout_override.as_ref().and_then(|layout| {
                    settings
                        .workspace
                        .presets
                        .iter()
                        .find(|preset| {
                            preset.layout.tab_layout() == layout.tab_layout()
                                && preset.layout.sidebar_side() == layout.sidebar_side()
                        })
                        .map(|preset| preset.id.clone())
                })
            })
        });
        let selected_layout = preset_id
            .as_deref()
            .and_then(|id| {
                settings
                    .workspace
                    .presets
                    .iter()
                    .find(|preset| preset.id == id)
            })
            .or_else(|| settings.workspace.default_preset())
            .map(|preset| preset.layout.clone())
            .unwrap_or_default();
        let name = cx.new(|cx| {
            let mut state = EditableTextState::new(StringStorage::default(), cx);
            if let Some(space) = space {
                state.emplace(&space.name, cx);
            }
            state
        });
        let preset_name = cx.new(|cx| EditableTextState::new(StringStorage::default(), cx));
        window.focus(&name.focus_handle(cx), cx);
        name.update(cx, |name, cx| name.select_document(cx));
        let name_observer = cx.observe(&name, |_, _, cx| cx.notify());
        let preset_name_observer = cx.observe(&preset_name, |_, _, cx| cx.notify());
        Self {
            creating: space.is_none(),
            name,
            color: space.map_or(WindowColor::System, |space| space.color),
            gradient: space.is_some_and(|space| space.gradient),
            preset_id,
            creating_preset: false,
            preset_name,
            preset_tab_layout: selected_layout.tab_layout(),
            preset_sidebar_side: selected_layout.sidebar_side(),
            invalid_preset: false,
            _name_observer: name_observer,
            _preset_name_observer: preset_name_observer,
        }
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let new_preset = if self.creating_preset {
            let name = self.preset_name.read(cx).as_str().trim().to_owned();
            let layout = WorkspaceLayout::for_tab_layout_and_side(
                self.preset_tab_layout,
                self.preset_sidebar_side,
            );
            let mut workspace = Settings::get(cx).workspace.clone();
            if workspace.add_preset(&name, layout.clone()).is_none() {
                self.invalid_preset = true;
                cx.notify();
                return;
            }
            Some((name, layout))
        } else {
            None
        };
        cx.emit(SpaceEditorEvent::Save {
            name: self.name.read(cx).as_str().trim().to_owned(),
            color: self.color,
            gradient: self.gradient,
            preset_id: if new_preset.is_some() {
                None
            } else {
                self.preset_id.clone()
            },
            new_preset,
        });
    }

    fn select_preset(&mut self, preset_id: Option<String>, cx: &mut Context<Self>) {
        self.preset_id = preset_id;
        self.creating_preset = false;
        self.invalid_preset = false;
        cx.notify();
    }

    fn start_preset_creation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let layout = self
            .preset_id
            .as_deref()
            .and_then(|id| {
                Settings::get(cx)
                    .workspace
                    .presets
                    .iter()
                    .find(|preset| preset.id == id)
            })
            .or_else(|| Settings::get(cx).workspace.default_preset())
            .map(|preset| preset.layout.clone())
            .unwrap_or_default();
        self.preset_tab_layout = layout.tab_layout();
        self.preset_sidebar_side = layout.sidebar_side();
        self.creating_preset = true;
        self.invalid_preset = false;
        window.focus(&self.preset_name.focus_handle(cx), cx);
        cx.notify();
    }

    fn cancel_preset_creation(&mut self, cx: &mut Context<Self>) {
        self.creating_preset = false;
        self.invalid_preset = false;
        cx.notify();
    }

    fn set_preset_tab_layout(&mut self, layout: TabLayout, cx: &mut Context<Self>) {
        self.preset_tab_layout = layout;
        self.invalid_preset = false;
        cx.notify();
    }

    fn set_preset_sidebar_side(&mut self, side: SidebarSide, cx: &mut Context<Self>) {
        self.preset_sidebar_side = side;
        self.invalid_preset = false;
        cx.notify();
    }
}

impl Focusable for SpaceEditor {
    fn focus_handle(&self, cx: &gpui::App) -> FocusHandle {
        self.name.focus_handle(cx)
    }
}

impl Render for SpaceEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = palette(window, cx);
        let (title, confirm) = if self.creating {
            ("New space", "Create")
        } else {
            ("Edit space", "Save")
        };
        let this = cx.entity().downgrade();
        let colors = color_picker(
            "space-editor-color",
            self.color,
            self.gradient,
            palette,
            Rc::new(move |color, gradient, _, cx| {
                this.update(cx, |this, cx| {
                    this.color = color;
                    this.gradient = gradient;
                    cx.notify();
                })
                .ok();
            }),
        );
        let settings = Settings::get(cx);
        let mut preset_choices = vec![Choice {
            label: "Use profile default".into(),
            chosen: self.preset_id.is_none(),
            on_choose: Box::new(cx.listener(|this, _, _, cx| this.select_preset(None, cx))),
        }];
        preset_choices.extend(settings.workspace.presets.iter().map(|preset| {
            let id = preset.id.clone();
            Choice {
                label: preset.name.clone().into(),
                chosen: self.preset_id.as_deref() == Some(&id),
                on_choose: Box::new(
                    cx.listener(move |this, _, _, cx| this.select_preset(Some(id.clone()), cx)),
                ),
            }
        }));
        let preset_picker = h_stack()
            .items_center()
            .gap(px(metrics::BUTTON_GAP))
            .child(dropdown(
                "space-editor-layout-preset",
                "Layout preset",
                preset_choices,
                palette,
            ))
            .child(button(
                "space-editor-new-preset",
                if self.creating_preset {
                    "Cancel new preset"
                } else {
                    "New preset…"
                },
                ButtonKind::Secondary,
                true,
                ButtonSize::Regular,
                palette,
                Box::new(cx.listener(|this, _, window, cx| {
                    if this.creating_preset {
                        this.cancel_preset_creation(cx);
                    } else {
                        this.start_preset_creation(window, cx);
                    }
                })),
            ));
        let preset_creation = self.creating_preset.then(|| {
            let name_field = form_field(
                "Preset name",
                text_field(
                    text_input("space-editor-preset-name")
                        .state(self.preset_name.downgrade())
                        .track_focus(&self.preset_name.focus_handle(cx))
                        .placeholder("Name")
                        .placeholder_color(rgb(palette.text_secondary)),
                    palette,
                ),
                palette,
            );
            let layout_fields = super::workspace_preset_fields::fields(
                "space-editor-preset-tabs",
                "space-editor-preset-sidebar",
                self.preset_tab_layout,
                self.preset_sidebar_side,
                palette,
                |layout| {
                    Box::new(
                        cx.listener(move |this, _, _, cx| this.set_preset_tab_layout(layout, cx)),
                    )
                },
                |side| {
                    Box::new(
                        cx.listener(move |this, _, _, cx| this.set_preset_sidebar_side(side, cx)),
                    )
                },
            );
            let form = v_stack()
                .gap(px(metrics::INTERNAL_PAGE_GROUP_GAP))
                .child(
                    div()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child("Create a preset for this space"),
                )
                .child(name_field)
                .child(layout_fields);
            if self.invalid_preset {
                form.child(error_text(
                    "Enter a name of up to 64 characters. The profile can have up to 32 presets.",
                    palette,
                ))
            } else {
                form
            }
        });
        modal_panel(palette)
            .id("space-editor")
            .role(Role::Dialog)
            .aria_label(title)
            .aria_modal(true)
            .text_size(px(metrics::INTERNAL_PAGE_BODY_SIZE))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.modifiers.modified() {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "enter" => this.save(cx),
                    "escape" => cx.emit(SpaceEditorEvent::Cancel),
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .child(dialog_title(title, palette))
            .child(form_field(
                "Space name",
                text_field(
                    text_input("space-editor-name")
                        .state(self.name.downgrade())
                        .track_focus(&self.name.focus_handle(cx))
                        .placeholder("Name")
                        .placeholder_color(rgb(palette.text_secondary)),
                    palette,
                ),
                palette,
            ))
            .child(form_field("Window colour", colors, palette))
            .child(form_field("Layout preset", preset_picker, palette))
            .children(preset_creation)
            .child(
                h_stack()
                    .justify_end()
                    .gap(px(metrics::BUTTON_GAP))
                    .child(button(
                        "space-editor-cancel",
                        "Cancel",
                        ButtonKind::Secondary,
                        true,
                        ButtonSize::Regular,
                        palette,
                        Box::new(cx.listener(|_, _, _, cx| cx.emit(SpaceEditorEvent::Cancel))),
                    ))
                    .child(button(
                        "space-editor-save",
                        confirm,
                        ButtonKind::Primary,
                        true,
                        ButtonSize::Regular,
                        palette,
                        Box::new(cx.listener(|this, _, _, cx| this.save(cx))),
                    )),
            )
    }
}
