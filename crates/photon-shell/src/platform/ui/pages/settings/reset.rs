//! Resetting every setting to its default, after asking.

use gpui::{AnyElement, Context, KeyDownEvent, Role, Window, div, prelude::*, px, rgb};

use super::super::super::button::{ButtonSize, button};
use super::super::super::layout::h_stack;
use super::super::super::modal::{MODAL_MOTION, modal, modal_panel};
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
            false,
            ButtonSize::Small,
            palette,
            Box::new(cx.listener(|this, _, window, cx| this.ask_to_reset(window, cx))),
        )
    }

    fn ask_to_reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.reset_focus, cx);
        self.set_confirming_reset(true, cx);
    }

    fn set_confirming_reset(&mut self, confirming: bool, cx: &mut Context<Self>) {
        self.confirming_reset = confirming;
        cx.notify();
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        Settings::update(cx, |settings| settings.reset_preferences());
        self.set_confirming_reset(false, cx);
    }

    /// The dialog asking to reset, while open or closing.
    pub(super) fn reset_dialog(
        &mut self,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let ((), transition) =
            self.reset_presence
                .sync(self.confirming_reset.then_some(()), MODAL_MOTION, cx)?;
        let panel = modal_panel(palette)
            .id("settings-reset-dialog")
            .role(Role::AlertDialog)
            .aria_label("Reset settings")
            .aria_modal(true)
            .track_focus(&self.reset_focus)
            .text_size(px(metrics::INTERNAL_PAGE_BODY_SIZE))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.modifiers.modified() {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "enter" => this.reset(cx),
                    "escape" => this.set_confirming_reset(false, cx),
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .child(
                div()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(rgb(palette.text_primary))
                    .child("Reset all settings?"),
            )
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
                        false,
                        ButtonSize::Regular,
                        palette,
                        Box::new(
                            cx.listener(|this, _, _, cx| this.set_confirming_reset(false, cx)),
                        ),
                    ))
                    .child(button(
                        "settings-reset-confirm",
                        "Reset",
                        true,
                        ButtonSize::Regular,
                        palette,
                        Box::new(cx.listener(|this, _, _, cx| this.reset(cx))),
                    )),
            );
        Some(modal("settings-reset-modal", transition, palette, panel).into_any_element())
    }
}
