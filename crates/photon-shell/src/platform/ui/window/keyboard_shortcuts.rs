//! The keyboard shortcuts dialog over the window.

use gpui::{AnyElement, Context, Focusable, Window, div, prelude::*};

use super::super::modal::{MODAL_MOTION, modal};
use super::super::shortcuts_dialog::{ShortcutsDialog, ShortcutsDialogEvent};
use super::super::theme::ThemeColors;
use super::BrowserWindow;

impl BrowserWindow {
    /// Opens the keyboard shortcuts dialog, or focuses it when it is open.
    pub(super) fn show_keyboard_shortcuts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dialog = match &self.shortcuts_dialog {
            Some((dialog, _)) => dialog.clone(),
            None => {
                let dialog = cx.new(ShortcutsDialog::new);
                let subscription = cx.subscribe(&dialog, |this, _, event, cx| match event {
                    ShortcutsDialogEvent::Dismiss => {
                        this.shortcuts_dialog = None;
                        cx.notify();
                    }
                });
                self.shortcuts_dialog = Some((dialog.clone(), subscription));
                cx.notify();
                dialog
            }
        };
        window.focus(&dialog.focus_handle(cx), cx);
    }

    /// The open dialog, or one closing, over the window.
    pub(super) fn shortcuts_dialog_overlay(
        &mut self,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let animations = super::super::settings::Settings::get(cx).animations;
        let open = self
            .shortcuts_dialog
            .as_ref()
            .map(|(dialog, _)| dialog.clone());
        let (dialog, transition) =
            self.shortcuts_dialog_presence
                .sync(open, MODAL_MOTION, &animations, cx)?;
        Some(
            modal(
                "keyboard-shortcuts-modal",
                transition,
                palette,
                div().child(dialog),
                cx,
            )
            .into_any_element(),
        )
    }
}
