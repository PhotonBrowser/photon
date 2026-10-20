//! Opening the command palette and executing a selected browser command.

use gpui::{AnyElement, Context, Window, div, prelude::*};

use super::super::command_palette::{COMMAND_PALETTE_MOTION, CommandPalette, CommandPaletteEvent};
use super::super::modal::modal;
use super::super::settings::Settings;
use super::BrowserWindow;

impl BrowserWindow {
    /// Opens the command palette, or focuses its search field when already open.
    pub(super) fn open_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let availability = self.command_availability(cx);
        if let Some(palette) = self.command_palette.clone() {
            palette.update(cx, |palette, cx| {
                palette.set_availability(availability, cx);
                palette.focus(window, cx);
            });
            return;
        }

        let palette = cx.new(|cx| CommandPalette::new(availability, window, cx));
        self._command_palette_subscription = Some(cx.subscribe_in(
            &palette,
            window,
            |this, _, event: &CommandPaletteEvent, window, cx| {
                this.command_palette_event(event, window, cx);
            },
        ));
        self.command_palette = Some(palette);
        cx.notify();
    }

    /// The open palette, or one closing, drawn over the window.
    pub(super) fn command_palette_overlay(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let availability = self.command_availability(cx);
        if let Some(palette) = self.command_palette.clone() {
            palette.update(cx, |palette, cx| palette.set_availability(availability, cx));
        }

        let animations = Settings::motion(cx);
        let (palette, transition) = self.command_palette_presence.sync(
            self.command_palette.clone(),
            COMMAND_PALETTE_MOTION,
            &animations,
            cx,
        )?;
        Some(
            modal(
                "command-palette-modal",
                transition,
                super::super::theme::palette(window, cx),
                div().child(palette),
                cx,
            )
            .into_any_element(),
        )
    }

    fn command_palette_event(
        &mut self,
        event: &CommandPaletteEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.command_palette = None;
        self._command_palette_subscription = None;
        self.restore_command_focus(window, cx);
        if let CommandPaletteEvent::Run(command) = event
            && self.command_availability(cx).allows(*command)
        {
            self.execute_command(*command, window, cx);
        }
        cx.notify();
    }

    fn restore_command_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(active_tab) = self.active_tab {
            self.activate_tab(active_tab, true, window, cx);
        } else {
            self.omnibox
                .update(cx, |omnibox, cx| omnibox.focus(window, cx));
        }
    }
}
