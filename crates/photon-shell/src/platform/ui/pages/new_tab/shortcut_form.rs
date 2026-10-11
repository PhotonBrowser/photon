//! The form for adding a shortcut by name and address.

use gpui::{Context, Entity, Focusable, Role, Window, prelude::*, px, rgb};
use gpui_elements::editable_text::{
    EditableTextState, StringStorage,
    actions::{Enter, Escape},
    text_input,
};
use photon_core::{OmniboxTarget, Shortcut, resolve_omnibox_input, site_name};

use super::super::super::button::{ButtonKind, ButtonSize, button};
use super::super::super::controls::text_field;
use super::super::super::layout::{error_text, h_stack};
use super::super::super::modal::{MODAL_MOTION, dialog_title, modal, modal_panel};
use super::super::super::settings::Settings;
use super::super::super::{metrics, theme::ThemeColors};
use super::NewTabPage;

/// The name and address of a shortcut being added.
#[derive(Clone)]
pub(super) struct ShortcutForm {
    name: Entity<EditableTextState>,
    address: Entity<EditableTextState>,
    /// The address cannot be opened. Cleared by the next attempt.
    invalid: bool,
}

impl NewTabPage {
    pub(super) fn start_adding(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = cx.new(|cx| EditableTextState::new(StringStorage::default(), cx));
        let address = cx.new(|cx| EditableTextState::new(StringStorage::default(), cx));
        // Redraw the form for every edit, caret move and blink.
        cx.observe(&name, |_, _, cx| cx.notify()).detach();
        cx.observe(&address, |_, _, cx| cx.notify()).detach();
        window.focus(&name.focus_handle(cx), cx);
        self.customising = false;
        self.adding = Some(ShortcutForm {
            name,
            address,
            invalid: false,
        });
        cx.notify();
    }

    fn cancel_adding(&mut self, cx: &mut Context<Self>) {
        self.adding = None;
        cx.notify();
    }

    /// Pins the form's site, or marks the address when it cannot be opened.
    fn finish_adding(&mut self, cx: &mut Context<Self>) {
        let Some(form) = self.adding.as_mut() else {
            return;
        };
        let address = form.address.read(cx).as_str().to_owned();
        let name = form.name.read(cx).as_str().trim().to_owned();
        let Ok(OmniboxTarget::Url { url, .. }) = resolve_omnibox_input(&address) else {
            form.invalid = true;
            cx.notify();
            return;
        };
        let title = if name.is_empty() {
            site_name(&url)
        } else {
            name
        };
        Settings::update(cx, |settings| settings.new_tab.pin(Shortcut { title, url }));
        self.adding = None;
        cx.notify();
    }

    /// A modal asking for the shortcut's name and address.
    pub(super) fn shortcut_form(
        &mut self,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        // A closed form stays drawn while it fades away.
        let (form, transition) =
            self.adding_presence
                .sync(self.adding.clone(), MODAL_MOTION, cx)?;
        let form = &form;
        let field = |id: &'static str, state: &Entity<EditableTextState>, placeholder| {
            text_field(
                text_input(id)
                    .state(state.downgrade())
                    .track_focus(&state.focus_handle(cx))
                    .placeholder(placeholder)
                    .placeholder_color(rgb(palette.text_secondary)),
                palette,
            )
        };
        let mut panel = modal_panel(palette)
            .id("new-tab-shortcut-form")
            .role(Role::Dialog)
            .aria_label("Add shortcut")
            .aria_modal(true)
            .capture_action(cx.listener(|this, _: &Enter, _, cx| {
                cx.stop_propagation();
                this.finish_adding(cx);
            }))
            .capture_action(cx.listener(|this, _: &Escape, _, cx| {
                cx.stop_propagation();
                this.cancel_adding(cx);
            }))
            .child(dialog_title("Add shortcut", palette))
            .child(field("new-tab-shortcut-name", &form.name, "Name"))
            .child(field("new-tab-shortcut-address", &form.address, "Address"));
        if form.invalid {
            panel = panel.child(error_text("This address can't be opened", palette));
        }
        let buttons = h_stack()
            .justify_end()
            .gap(px(metrics::BUTTON_GAP))
            .child(button(
                "new-tab-shortcut-cancel",
                "Cancel",
                ButtonKind::Secondary,
                true,
                ButtonSize::Regular,
                palette,
                Box::new(cx.listener(|this, _, _, cx| this.cancel_adding(cx))),
            ))
            .child(button(
                "new-tab-shortcut-add",
                "Add",
                ButtonKind::Primary,
                true,
                ButtonSize::Regular,
                palette,
                Box::new(cx.listener(|this, _, _, cx| this.finish_adding(cx))),
            ));
        Some(modal(
            "new-tab-shortcut-modal",
            transition,
            palette,
            panel.child(buttons),
        ))
    }
}
