//! Keyboard shortcuts and Escape handling for the browser window.

use gpui::{Context, Div, KeyDownEvent, Window, prelude::*};
use photon_core::Command;
use photon_shortcuts::{
    CloseTab, FindInPage, FindNext, FindPrevious, FocusOmnibox, GoBack, GoForward, MinimizeWindow,
    NewTab, NextSpace, OpenCommandPalette, OpenSettings, PreviousSpace, Reload, ReopenClosedTab,
    ResetZoom, SelectLastTab, SelectNextTab, SelectPreviousTab, SelectTab1, SelectTab2, SelectTab3,
    SelectTab4, SelectTab5, SelectTab6, SelectTab7, SelectTab8, ShowKeyboardShortcuts, StopLoading,
    ToggleDebugInfo, ToggleSidebar, ToggleSidebarIcons, ToggleZenMode, ZoomIn, ZoomOut, ZoomWindow,
};

use super::BrowserWindow;

impl BrowserWindow {
    /// Handles the browser's shortcuts and Escape on the window's root element.
    pub(super) fn on_actions(&self, root: Div, cx: &mut Context<Self>) -> Div {
        root.on_key_down(cx.listener(Self::escape))
            .on_action(cx.listener(|this, _: &OpenCommandPalette, window, cx| {
                this.open_command_palette(window, cx);
            }))
            .on_action(cx.listener(|this, _: &FocusOmnibox, window, cx| {
                this.execute_command(Command::FocusAddressField, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleSidebar, window, cx| {
                this.execute_command(Command::ToggleSidebar, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleSidebarIcons, window, cx| {
                this.execute_command(Command::ToggleSidebarIcons, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleZenMode, window, cx| {
                this.execute_command(Command::ToggleZenMode, window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenSettings, window, cx| {
                this.execute_command(Command::OpenSettings, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ShowKeyboardShortcuts, window, cx| {
                this.execute_command(Command::ShowKeyboardShortcuts, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleDebugInfo, _, cx| {
                this.toggle_performance_overlay(cx);
            }))
            .on_action(cx.listener(|this, _: &NextSpace, window, cx| {
                this.execute_command(Command::NextSpace, window, cx);
            }))
            .on_action(cx.listener(|this, _: &PreviousSpace, window, cx| {
                this.execute_command(Command::PreviousSpace, window, cx);
            }))
            .on_action(cx.listener(|this, _: &FindInPage, window, cx| {
                this.execute_command(Command::FindInPage, window, cx);
            }))
            .on_action(cx.listener(|this, _: &FindNext, window, cx| {
                this.find_step(true, window, cx);
            }))
            .on_action(cx.listener(|this, _: &FindPrevious, window, cx| {
                this.find_step(false, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ZoomIn, window, cx| {
                this.execute_command(Command::ZoomIn, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ZoomOut, window, cx| {
                this.execute_command(Command::ZoomOut, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ResetZoom, window, cx| {
                this.execute_command(Command::ResetZoom, window, cx);
            }))
            .on_action(cx.listener(|this, _: &NewTab, window, cx| {
                this.execute_command(Command::NewTab, window, cx);
            }))
            .on_action(|_: &MinimizeWindow, window, _| window.minimize_window())
            .on_action(|_: &ZoomWindow, window, _| window.zoom_window())
            .on_action(cx.listener(|this, _: &Reload, window, cx| {
                this.execute_command(Command::ReloadPage, window, cx);
            }))
            .on_action(cx.listener(|this, _: &StopLoading, window, cx| {
                this.execute_command(Command::StopLoading, window, cx);
            }))
            .on_action(cx.listener(|this, _: &GoBack, window, cx| {
                this.execute_command(Command::GoBack, window, cx);
            }))
            .on_action(cx.listener(|this, _: &GoForward, window, cx| {
                this.execute_command(Command::GoForward, window, cx);
            }))
            .on_action(cx.listener(|this, _: &CloseTab, window, cx| {
                this.execute_command(Command::CloseTab, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ReopenClosedTab, window, cx| {
                this.execute_command(Command::ReopenClosedTab, window, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectNextTab, window, cx| {
                this.execute_command(Command::SelectNextTab, window, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectPreviousTab, window, cx| {
                this.execute_command(Command::SelectPreviousTab, window, cx);
            }))
            .on_action(cx.listener(select_tab_at::<SelectTab1>(0)))
            .on_action(cx.listener(select_tab_at::<SelectTab2>(1)))
            .on_action(cx.listener(select_tab_at::<SelectTab3>(2)))
            .on_action(cx.listener(select_tab_at::<SelectTab4>(3)))
            .on_action(cx.listener(select_tab_at::<SelectTab5>(4)))
            .on_action(cx.listener(select_tab_at::<SelectTab6>(5)))
            .on_action(cx.listener(select_tab_at::<SelectTab7>(6)))
            .on_action(cx.listener(select_tab_at::<SelectTab8>(7)))
            .on_action(cx.listener(|this, _: &SelectLastTab, window, cx| {
                if let Some(&last) = this.listed_tabs(cx).last() {
                    this.activate_tab(last, true, window, cx);
                }
            }))
    }

    /// Escape dismisses a notice chip. Menus and dialogs handle their own
    /// Escape before it gets here.
    fn escape(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key != "escape" {
            return;
        }
        if !self.dismiss_notice(cx) {
            return;
        }
        window.prevent_default();
        cx.stop_propagation();
        cx.notify();
    }
}

/// An action handler that activates the listed tab at `position`, if there
/// is one.
fn select_tab_at<A>(
    position: usize,
) -> impl Fn(&mut BrowserWindow, &A, &mut Window, &mut Context<BrowserWindow>) {
    move |this, _, window, cx| {
        if let Some(&index) = this.listed_tabs(cx).get(position) {
            this.activate_tab(index, true, window, cx);
        }
    }
}
