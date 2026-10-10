//! The dialog for creating a space or changing one: its name and its window
//! colour. The window shows it as a modal and carries out the result.

use gpui::{
    Context, Entity, EventEmitter, FocusHandle, Focusable, KeyDownEvent, Render, Role,
    Subscription, Window, div, prelude::*, px, rgb,
};
use gpui_elements::editable_text::{EditableTextState, StringStorage, text_input};
use photon_core::{Space, WindowColor};
use std::rc::Rc;

use super::button::{ButtonSize, button};
use super::color_picker::color_picker;
use super::controls::text_field;
use super::layout::h_stack;
use super::modal::modal_panel;
use super::{metrics, theme::palette};

/// What the dialog asks its window to do. Each closes it.
pub(super) enum SpaceEditorEvent {
    Save {
        name: String,
        color: WindowColor,
        gradient: bool,
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
    _name_observer: Subscription,
}

impl SpaceEditor {
    /// A dialog for changing `space`, or for a new space when `None`.
    pub(super) fn new(space: Option<&Space>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| {
            let mut state = EditableTextState::new(StringStorage::default(), cx);
            if let Some(space) = space {
                state.emplace(&space.name, cx);
            }
            state
        });
        window.focus(&name.focus_handle(cx), cx);
        name.update(cx, |name, cx| name.select_document(cx));
        let name_observer = cx.observe(&name, |_, _, cx| cx.notify());
        Self {
            creating: space.is_none(),
            name,
            color: space.map_or(WindowColor::System, |space| space.color),
            gradient: space.is_some_and(|space| space.gradient),
            _name_observer: name_observer,
        }
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        cx.emit(SpaceEditorEvent::Save {
            name: self.name.read(cx).as_str().trim().to_owned(),
            color: self.color,
            gradient: self.gradient,
        });
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
            .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(title))
            .child(text_field(
                text_input("space-editor-name")
                    .state(self.name.downgrade())
                    .track_focus(&self.name.focus_handle(cx))
                    .placeholder("Name")
                    .placeholder_color(rgb(palette.text_secondary)),
                palette,
            ))
            .child(colors)
            .child(
                h_stack()
                    .justify_end()
                    .gap(px(metrics::BUTTON_GAP))
                    .child(button(
                        "space-editor-cancel",
                        "Cancel",
                        false,
                        ButtonSize::Regular,
                        palette,
                        Box::new(cx.listener(|_, _, _, cx| cx.emit(SpaceEditorEvent::Cancel))),
                    ))
                    .child(button(
                        "space-editor-save",
                        confirm,
                        true,
                        ButtonSize::Regular,
                        palette,
                        Box::new(cx.listener(|this, _, _, cx| this.save(cx))),
                    )),
            )
    }
}
