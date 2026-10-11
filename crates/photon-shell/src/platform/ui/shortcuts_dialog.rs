//! A dialog listing the browser's keyboard shortcuts by group, each with
//! the keys bound to it now. Escape or its close button dismisses it.

use gpui::{
    Context, EventEmitter, FocusHandle, Focusable, KeyDownEvent, Render, Role, Window, div,
    prelude::*, px, rgb, rgba,
};
use photon_shortcuts::{ShortcutGroup, shortcut_groups};

use super::button::icon_button;
use super::icons::close_icon;
use super::kbd::action_kbd;
use super::layout::{h_stack, v_stack};
use super::modal::{dialog_title, modal_panel};
use super::{metrics, theme::ThemeColors, theme::palette};

pub(super) enum ShortcutsDialogEvent {
    Dismiss,
}

pub(super) struct ShortcutsDialog {
    focus_handle: FocusHandle,
}

impl EventEmitter<ShortcutsDialogEvent> for ShortcutsDialog {}

impl ShortcutsDialog {
    pub(super) fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for ShortcutsDialog {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ShortcutsDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = palette(window, cx);
        let header = h_stack()
            .items_center()
            .justify_between()
            .child(dialog_title("Keyboard shortcuts", palette))
            .child(icon_button(
                "keyboard-shortcuts-close",
                "Close",
                true,
                metrics::ICON_BUTTON_SIZE,
                close_icon(palette.text_secondary, metrics::ICON_BUTTON_ICON_SIZE),
                palette,
                Box::new(cx.listener(|_, _, _, cx| cx.emit(ShortcutsDialogEvent::Dismiss))),
            ));
        let groups = shortcut_groups()
            .into_iter()
            .map(|group| shortcut_group(group, palette, cx));
        modal_panel(palette)
            .id("keyboard-shortcuts")
            .role(Role::Dialog)
            .aria_label("Keyboard Shortcuts")
            .aria_modal(true)
            .w(px(metrics::SHORTCUTS_DIALOG_WIDTH))
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|_, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" && !event.keystroke.modifiers.modified() {
                    cx.stop_propagation();
                    cx.emit(ShortcutsDialogEvent::Dismiss);
                }
            }))
            .child(header)
            .child(
                v_stack()
                    .id("keyboard-shortcuts-list")
                    .max_h(px(metrics::SHORTCUTS_DIALOG_MAX_HEIGHT))
                    .overflow_y_scroll()
                    .gap(px(metrics::INTERNAL_PAGE_GROUP_GAP))
                    .children(groups),
            )
    }
}

/// A group's name over its shortcuts, one row each: what it does, then its
/// keys.
fn shortcut_group(
    group: ShortcutGroup,
    palette: ThemeColors,
    cx: &gpui::App,
) -> gpui::Stateful<gpui::Div> {
    let rows = group.entries.into_iter().map(|entry| {
        h_stack()
            .items_center()
            .justify_between()
            .gap(px(metrics::MENU_ITEM_GAP))
            .h(px(metrics::MENU_ITEM_HEIGHT))
            .text_size(px(metrics::MENU_FONT_SIZE))
            .child(div().min_w_0().truncate().child(entry.name))
            .children(action_kbd(entry.action.as_ref(), palette, cx))
    });
    v_stack()
        .id(gpui::ElementId::Name(group.name.into()))
        .role(Role::Group)
        .aria_label(group.name)
        .child(
            div()
                .pb(px(metrics::MENU_ITEM_GAP))
                .mb(px(metrics::MENU_ITEM_GAP))
                .border_b_1()
                .border_color(rgba(palette.menu_border))
                .text_size(px(metrics::TAB_FONT_SIZE))
                .text_color(rgb(palette.text_secondary))
                .child(group.name),
        )
        .children(rows)
}
