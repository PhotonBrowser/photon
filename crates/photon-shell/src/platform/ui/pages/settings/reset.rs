//! Resetting every setting to its default, after asking.

use gpui::{AnyElement, Context, KeyDownEvent, Role, Window, prelude::*, px};

use super::super::super::button::{ButtonKind, ButtonSize, button};
use super::super::super::layout::h_stack;
use super::super::super::modal::{MODAL_MOTION, dialog_title, modal, modal_panel};
use super::super::super::settings::Settings;
use super::super::super::{metrics, theme::ThemeColors};
use super::super::layout::secondary_text;
use super::SettingsPage;

impl SettingsPage {
    /// The button at the foot of the sidebar that asks to reset.
    pub(super) fn reset_button(
        &self,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        button(
            "settings-reset",
            "Reset to defaults",
            ButtonKind::Secondary,
            true,
            ButtonSize::Small,
            palette,
            Box::new(cx.listener(|this, _, window, cx| this.ask_to_reset(window, cx))),
        )
    }

    fn ask_to_reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.remember_dialog_focus(window, cx);
        window.focus(&self.reset_focus, cx);
        self.confirming_reset = true;
        cx.notify();
    }

    /// Closes the dialog, giving focus back to what opened it.
    fn close_reset_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.confirming_reset = false;
        self.restore_dialog_focus(window, cx);
        cx.notify();
    }

    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        Settings::update(cx, |settings| settings.reset_preferences());
        super::super::super::show_alert(cx, "Settings reset");
        self.close_reset_dialog(window, cx);
    }

    /// The dialog asking to reset, while open or closing.
    pub(super) fn reset_dialog(
        &mut self,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let animations = Settings::motion(cx);
        let ((), transition) = self.reset_presence.sync(
            self.confirming_reset.then_some(()),
            MODAL_MOTION,
            &animations,
            cx,
        )?;
        let panel = modal_panel(palette)
            .id("settings-reset-dialog")
            .role(Role::AlertDialog)
            .aria_label("Reset settings")
            .aria_modal(true)
            .track_focus(&self.reset_focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.modifiers.modified() {
                    return;
                }
                // Enter does not reset: only the button does.
                if event.keystroke.key == "escape" {
                    this.close_reset_dialog(window, cx);
                    cx.stop_propagation();
                }
            }))
            .child(dialog_title("Reset all settings?", palette))
            .child(secondary_text(
                "Appearance, layout, search, the new tab page, tabs and sites go back to \
                 their defaults. Your favourites and shortcuts stay.",
                palette,
            ))
            .child(
                h_stack()
                    .justify_end()
                    .gap(px(metrics::BUTTON_GAP))
                    .child(button(
                        "settings-reset-cancel",
                        "Cancel",
                        ButtonKind::Secondary,
                        true,
                        ButtonSize::Regular,
                        palette,
                        Box::new(
                            cx.listener(|this, _, window, cx| this.close_reset_dialog(window, cx)),
                        ),
                    ))
                    .child(button(
                        "settings-reset-confirm",
                        "Reset",
                        ButtonKind::Destructive,
                        true,
                        ButtonSize::Regular,
                        palette,
                        Box::new(cx.listener(|this, _, window, cx| this.reset(window, cx))),
                    )),
            );
        Some(modal("settings-reset-modal", transition, palette, panel, cx).into_any_element())
    }
}
