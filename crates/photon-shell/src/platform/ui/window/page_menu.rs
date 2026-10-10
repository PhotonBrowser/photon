//! The page's right-click menu, built from the items the Engine offers.

use gpui::{Context, prelude::*, px};

use super::super::layout::v_stack;
use super::super::menu::{menu_action, menu_checkbox, menu_disabled, menu_separator, menu_surface};
use super::super::motion::Transition;
use super::super::{PageMenuItem, metrics, theme::ThemeColors};
use super::{BrowserWindow, OpenMenu};

impl BrowserWindow {
    /// Opens the active page's context menu where the page asked for it.
    pub(super) fn open_page_menu(&mut self, cx: &mut Context<Self>) {
        let position = self
            .active_webview()
            .read(cx)
            .context_menu
            .as_ref()
            .map(|menu| menu.position);
        if let Some(position) = position {
            self.open_menu = Some(OpenMenu::Page(position));
            cx.notify();
        }
    }

    pub(super) fn page_menu(
        &self,
        transition: Transition,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let webview = self.active_webview().read(cx);
        let items = webview.context_menu.iter().flat_map(|menu| &menu.items);
        let mut tab_index = 0;
        let rows = items.enumerate().map(|(index, item)| {
            let id = ("page-menu-item", index);
            let PageMenuItem::Action {
                label,
                enabled,
                checked,
            } = item
            else {
                return menu_separator(palette).into_any_element();
            };
            if !enabled {
                return menu_disabled(id, label.clone(), palette).into_any_element();
            }
            let activate = Box::new(cx.listener(move |this: &mut Self, _, _, cx| {
                cx.stop_propagation();
                this.open_menu = None;
                this.active_webview()
                    .update(cx, |view, _| view.activate_context_menu_item(index));
                cx.notify();
            }));
            tab_index += 1;
            match checked {
                Some(checked) => {
                    menu_checkbox(id, label.clone(), tab_index, *checked, activate, palette)
                        .into_any_element()
                }
                None => {
                    menu_action(id, label.clone(), tab_index, activate, palette).into_any_element()
                }
            }
        });
        let content = v_stack()
            .gap(px(metrics::MENU_ITEM_GAP))
            .children(rows.collect::<Vec<_>>());
        menu_surface(content, transition, palette)
    }
}
