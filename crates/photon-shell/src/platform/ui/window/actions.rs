//! Keyboard shortcuts and Escape handling for the browser window.

use gpui::{Context, Div, KeyDownEvent, Window, prelude::*};
use photon_core::BrowserCommand;
use photon_shortcuts::{
    CloseTab, FocusOmnibox, GoBack, GoForward, NewTab, NewWindow, Reload, ReopenClosedTab,
    SelectLastTab, SelectNextTab, SelectPreviousTab, SelectTab1, SelectTab2, SelectTab3,
    SelectTab4, SelectTab5, SelectTab6, SelectTab7, SelectTab8, StopLoading,
};

use super::BrowserWindow;

impl BrowserWindow {
    /// Handles the browser's shortcuts and Escape on the window's root element.
    pub(super) fn on_actions(&self, root: Div, cx: &mut Context<Self>) -> Div {
        root.on_key_down(cx.listener(Self::escape))
            .on_action(cx.listener(|this, _: &FocusOmnibox, window, cx| {
                this.omnibox
                    .update(cx, |omnibox, cx| omnibox.focus(window, cx));
            }))
            .on_action(cx.listener(|this, _: &NewTab, window, cx| {
                this.dispatch_command(BrowserCommand::NewTab, window, cx);
            }))
            .on_action(cx.listener(|this, _: &NewWindow, window, cx| {
                this.dispatch_command(BrowserCommand::NewWindow, window, cx);
            }))
            .on_action(cx.listener(|this, _: &Reload, window, cx| {
                this.dispatch_command(BrowserCommand::Reload, window, cx);
            }))
            .on_action(cx.listener(|this, _: &StopLoading, window, cx| {
                this.dispatch_command(BrowserCommand::StopLoading, window, cx);
            }))
            .on_action(cx.listener(|this, _: &GoBack, window, cx| {
                this.dispatch_command(BrowserCommand::Back, window, cx);
            }))
            .on_action(cx.listener(|this, _: &GoForward, window, cx| {
                this.dispatch_command(BrowserCommand::Forward, window, cx);
            }))
            .on_action(cx.listener(|this, _: &CloseTab, window, cx| {
                this.open_menu = None;
                this.close_tab(this.active_tab, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ReopenClosedTab, window, cx| {
                this.open_menu = None;
                this.reopen_closed_tab(window, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectNextTab, window, cx| {
                this.select_relative_tab(1, window, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectPreviousTab, window, cx| {
                this.select_relative_tab(-1, window, cx);
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
                this.activate_tab(this.tabs.len() - 1, true, window, cx);
            }))
    }

    /// Escape closes the open menu, or else dismisses a notice chip.
    fn escape(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key != "escape" {
            return;
        }
        if self.open_menu.take().is_none() && !self.dismiss_notice(cx) {
            return;
        }
        window.prevent_default();
        cx.stop_propagation();
        cx.notify();
    }
}

/// An action handler that activates the tab at `index`, if there is one.
fn select_tab_at<A>(
    index: usize,
) -> impl Fn(&mut BrowserWindow, &A, &mut Window, &mut Context<BrowserWindow>) {
    move |this, _, window, cx| this.activate_tab(index, true, window, cx)
}
