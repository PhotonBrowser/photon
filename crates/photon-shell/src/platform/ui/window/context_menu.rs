//! In-window GPUI context menus shared by browser chrome and web pages.

use gpui::{
    Action, AnyElement, Context, Entity, FocusHandle, KeyDownEvent, Menu, MenuItem, MouseButton,
    MouseDownEvent, Pixels, Point, Role, SharedString, WeakEntity, Window, deferred, div, point,
    prelude::*, px,
};

use super::super::menu::{menu_action_with_state, menu_heading, menu_separator, popover_surface};
use super::super::motion::Transition;
use super::super::settings::Settings;
use super::super::{PhotonWebView, SelectDropdown, SelectItem, metrics, theme::ThemeColors};
use super::BrowserWindow;
use super::menu::{MenuCommand, command_item};

pub(super) struct ContextMenuState {
    position: Point<Pixels>,
    items: Vec<ContextMenuItem>,
    return_focus: FocusHandle,
}

pub(super) struct SelectDropdownState {
    position: Point<Pixels>,
    width: f32,
    dropdown: SelectDropdown,
    webview: WeakEntity<PhotonWebView>,
    return_focus: FocusHandle,
}

enum ContextMenuItem {
    Separator,
    Heading(SharedString),
    Action {
        label: SharedString,
        action: Box<dyn Action>,
        checked: bool,
        disabled: bool,
    },
}

struct MenuOverlay {
    position: Point<Pixels>,
    width: f32,
    label: &'static str,
    role: Role,
    palette: ThemeColors,
}

impl BrowserWindow {
    /// Opens a Photon-styled menu at the pointer, clamped to the window.
    pub(super) fn show_context_menu(
        &mut self,
        window: &mut Window,
        items: Vec<MenuItem>,
        cx: &mut Context<Self>,
    ) {
        let return_focus = self.take_popover_focus(window, cx);
        let mut entries = Vec::new();
        flatten_menu(items, &mut entries);
        if entries.is_empty() {
            window.focus(&return_focus, cx);
            return;
        }

        let viewport = window.viewport_size();
        let preferred = window.mouse_position();
        let width = metrics::MENU_WIDTH.min(f32::from(viewport.width));
        let height = menu_height(&entries).min(f32::from(viewport.height));
        let x = f32::from(preferred.x).clamp(0.0, f32::from(viewport.width) - width);
        let y = f32::from(preferred.y).clamp(0.0, f32::from(viewport.height) - height);
        self.context_menu = Some(ContextMenuState {
            position: point(px(x), px(y)),
            items: entries,
            return_focus,
        });
        window.focus(&self.context_menu_focus, cx);
        cx.notify();
    }

    /// Places a page's `<select>` options in the shared GPUI popup layer.
    pub(super) fn show_select_dropdown(
        &mut self,
        webview: Entity<PhotonWebView>,
        dropdown: SelectDropdown,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if dropdown.items.is_empty() {
            webview.update(cx, |view, _| view.session.select_dropdown_closed(None));
            return;
        }

        let return_focus = self.take_popover_focus(window, cx);
        let origin = webview.read(cx).viewport_origin();
        let viewport = window.viewport_size();
        let width = dropdown
            .minimum_width
            .max(metrics::MENU_WIDTH)
            .min(f32::from(viewport.width));
        let height = select_menu_height(&dropdown.items).min(f32::from(viewport.height));
        let anchor_x = origin.0 + dropdown.position.0;
        let anchor_y = origin.1 + dropdown.position.1;
        let x = anchor_x.clamp(0.0, f32::from(viewport.width) - width);
        let below = f32::from(viewport.height) - anchor_y;
        let preferred_y = if height <= below || anchor_y < height {
            anchor_y
        } else {
            anchor_y - height
        };
        let y = preferred_y.clamp(0.0, f32::from(viewport.height) - height);
        self.select_dropdown = Some(SelectDropdownState {
            position: point(px(x), px(y)),
            width,
            dropdown,
            webview: webview.downgrade(),
            return_focus,
        });
        window.focus(&self.context_menu_focus, cx);
        cx.notify();
    }

    fn close_select_dropdown(
        &mut self,
        selection: Option<u32>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(dropdown) = self.select_dropdown.take() {
            let _ = dropdown
                .webview
                .update(cx, |view, _| view.session.select_dropdown_closed(selection));
            window.focus(&dropdown.return_focus, cx);
            cx.notify();
        }
    }

    fn take_popover_focus(&mut self, window: &Window, cx: &mut Context<Self>) -> FocusHandle {
        let select = self.select_dropdown.take();
        let select_focus = select.map(|dropdown| {
            let _ = dropdown
                .webview
                .update(cx, |view, _| view.session.select_dropdown_closed(None));
            dropdown.return_focus
        });
        let menu_focus = self.context_menu.take().map(|menu| menu.return_focus);

        select_focus
            .or(menu_focus)
            .or_else(|| window.focused(cx))
            .unwrap_or_else(|| self.root_focus.clone())
    }

    /// Opens the browser's context menu on a right-click in empty chrome.
    pub(super) fn with_browser_menu(&self, area: gpui::Div, cx: &mut Context<Self>) -> gpui::Div {
        area.on_mouse_down(
            MouseButton::Right,
            cx.listener(|this, _: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                this.show_browser_menu(window, cx);
            }),
        )
    }

    pub(super) fn show_browser_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.theme_picker_anchor = Some(window.mouse_position());
        let items = self.browser_menu_items(cx);
        self.show_context_menu(window, items, cx);
    }

    /// Builds the custom in-window menu over browser chrome and page content.
    pub(super) fn context_menu_overlay(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let menu = self.context_menu.as_ref()?;
        let position = menu.position;
        let palette = self.palette(window, cx);
        let items = menu
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| self.context_menu_row(index, item, palette, cx))
            .collect::<Vec<_>>();
        let viewport = window.viewport_size();
        let width = metrics::MENU_WIDTH.min(f32::from(viewport.width));

        Some(
            self.menu_overlay(
                MenuOverlay {
                    position,
                    width,
                    label: "Context menu",
                    role: Role::Menu,
                    palette,
                },
                div()
                    .id("browser-context-menu-list")
                    .max_h(menu_content_max_height(viewport.height))
                    .overflow_y_scroll()
                    .children(items),
                cx,
            ),
        )
    }

    /// Builds the custom in-window popup for a page's `<select>` control.
    pub(super) fn select_dropdown_overlay(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let dropdown = self.select_dropdown.as_ref()?;
        let position = dropdown.position;
        let width = dropdown.width.min(f32::from(window.viewport_size().width));
        let viewport = window.viewport_size();
        let palette = self.palette(window, cx);
        let items = dropdown
            .dropdown
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| self.select_dropdown_row(index, item, palette, cx))
            .collect::<Vec<_>>();

        Some(
            self.menu_overlay(
                MenuOverlay {
                    position,
                    width,
                    label: "Choose an option",
                    role: Role::ListBox,
                    palette,
                },
                div()
                    .id("select-dropdown-list")
                    .max_h(menu_content_max_height(viewport.height))
                    .overflow_y_scroll()
                    .children(items),
                cx,
            ),
        )
    }

    fn menu_overlay(
        &mut self,
        overlay: MenuOverlay,
        content: impl IntoElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let popup = div()
            .absolute()
            .left(overlay.position.x)
            .top(overlay.position.y)
            .track_focus(&self.context_menu_focus)
            .tab_index(0)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.modifiers.modified() {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "escape" => this.close_context_popover(window, cx),
                    "down" => window.focus_next(cx),
                    "up" => window.focus_prev(cx),
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .child(popover_surface(
                overlay.label,
                overlay.role,
                overlay.width,
                content,
                Transition::Enter,
                overlay.palette,
                cx,
            ));
        let close = cx.listener(|this, _: &MouseDownEvent, window, cx| {
            this.close_context_popover(window, cx);
            cx.stop_propagation();
        });
        deferred(popup.occlude().on_mouse_down_out(close))
            .priority(8)
            .into_any_element()
    }

    fn context_menu_row(
        &self,
        index: usize,
        item: &ContextMenuItem,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match item {
            ContextMenuItem::Separator => menu_separator(palette).into_any_element(),
            ContextMenuItem::Heading(label) => {
                menu_heading(label.clone(), palette).into_any_element()
            }
            ContextMenuItem::Action {
                label,
                action,
                checked,
                disabled,
            } => {
                let action = action.boxed_clone();
                let click = Box::new(cx.listener(move |this, _, window, cx| {
                    this.close_context_menu(window, cx);
                    window.dispatch_action(action.as_ref().boxed_clone(), cx);
                }));
                menu_action_with_state(
                    ("browser-context-menu", index),
                    label.clone(),
                    Role::MenuItem,
                    *checked,
                    *disabled,
                    click,
                    palette,
                )
                .into_any_element()
            }
        }
    }

    fn close_context_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(menu) = self.context_menu.take() {
            window.focus(&menu.return_focus, cx);
            cx.notify();
        }
    }

    fn close_context_popover(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.select_dropdown.is_some() {
            self.close_select_dropdown(None, window, cx);
        } else {
            self.close_context_menu(window, cx);
        }
    }

    fn select_dropdown_row(
        &self,
        index: usize,
        item: &SelectItem,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match item {
            SelectItem::Option {
                label,
                id,
                selected,
                enabled,
            } => {
                let id = *id;
                let click = Box::new(cx.listener(move |this, _, window, cx| {
                    this.close_select_dropdown(Some(id), window, cx);
                }));
                menu_action_with_state(
                    ("select-dropdown-option", index),
                    label.clone(),
                    Role::ListBoxOption,
                    *selected,
                    !*enabled,
                    click,
                    palette,
                )
                .into_any_element()
            }
            SelectItem::GroupLabel(label) => {
                menu_heading(label.clone(), palette).into_any_element()
            }
            SelectItem::Separator => menu_separator(palette).into_any_element(),
        }
    }

    /// New tabs and windows and settings; for a web page, finding, zoom and
    /// its debug details; then how the sidebar and chrome show.
    fn browser_menu_items(&self, cx: &Context<Self>) -> Vec<MenuItem> {
        use crate::platform::shortcuts::{
            FindInPage, NewTab, NewWindow, OpenSettings, ResetZoom, ShowKeyboardShortcuts,
            ToggleDebugInfo, ToggleSidebarIcons, ToggleZenMode, ZoomIn, ZoomOut,
        };

        let mut items = vec![
            MenuItem::action("New Tab", NewTab),
            MenuItem::action("New Window", NewWindow),
            MenuItem::separator(),
        ];
        if let Some(webview) = self.active_webview() {
            let debug_info = webview.read(cx).performance_overlay_enabled;
            items.extend([
                MenuItem::action("Find in Page…", FindInPage),
                MenuItem::submenu(Menu::new("Zoom").items([
                    MenuItem::action("Zoom In", ZoomIn),
                    MenuItem::action("Zoom Out", ZoomOut),
                    MenuItem::action("Actual Size", ResetZoom),
                ])),
                MenuItem::action("Debug Info", ToggleDebugInfo).checked(debug_info),
                MenuItem::separator(),
            ]);
        }
        if super::super::workspace_layout::WorkspaceLayout::current(cx).has_sidebar() {
            let icons = self.sidebar.docked() && self.sidebar.collapsed();
            items.push(MenuItem::action("Sidebar as Icons", ToggleSidebarIcons).checked(icons));
        }
        items.extend([
            command_item(
                "Edit Theme…",
                MenuCommand::EditTheme(Settings::get(cx).spaces.active().id),
            ),
            MenuItem::separator(),
            MenuItem::action("Zen Mode", ToggleZenMode).checked(self.sidebar.zen()),
            MenuItem::separator(),
            MenuItem::action("Keyboard Shortcuts", ShowKeyboardShortcuts),
            MenuItem::action("Settings…", OpenSettings),
        ]);
        items
    }
}

fn flatten_menu(items: Vec<MenuItem>, output: &mut Vec<ContextMenuItem>) {
    for item in items {
        match item {
            MenuItem::Separator => output.push(ContextMenuItem::Separator),
            MenuItem::Submenu(menu) => {
                if !menu.items.is_empty() {
                    output.push(ContextMenuItem::Heading(menu.name));
                    flatten_menu(menu.items, output);
                }
            }
            MenuItem::Action {
                name,
                action,
                checked,
                disabled,
                ..
            } => output.push(ContextMenuItem::Action {
                label: name,
                action,
                checked,
                disabled,
            }),
            MenuItem::SystemMenu(menu) => output.push(ContextMenuItem::Heading(menu.name)),
        }
    }
}

fn menu_height(items: &[ContextMenuItem]) -> f32 {
    items.iter().fold(menu_surface_inset(), |height, item| {
        height
            + match item {
                ContextMenuItem::Separator => {
                    metrics::MENU_SEPARATOR_HEIGHT + metrics::MENU_SEPARATOR_MARGIN * 2.0
                }
                ContextMenuItem::Action { .. } | ContextMenuItem::Heading(_) => {
                    metrics::MENU_ITEM_HEIGHT
                }
            }
    })
}

fn select_menu_height(items: &[SelectItem]) -> f32 {
    items.iter().fold(menu_surface_inset(), |height, item| {
        height
            + match item {
                SelectItem::Separator => {
                    metrics::MENU_SEPARATOR_HEIGHT + metrics::MENU_SEPARATOR_MARGIN * 2.0
                }
                SelectItem::Option { .. } | SelectItem::GroupLabel(_) => metrics::MENU_ITEM_HEIGHT,
            }
    })
}

fn menu_surface_inset() -> f32 {
    metrics::MENU_PADDING * 2.0 + metrics::SURFACE_BORDER_WIDTH * 2.0
}

fn menu_content_max_height(viewport_height: Pixels) -> Pixels {
    px((f32::from(viewport_height) - menu_surface_inset()).max(0.0))
}

#[cfg(test)]
mod tests {
    use super::super::super::menu::menu_action_with_state;
    use crate::platform::ui::theme::palette;
    use gpui::{
        Context, FocusHandle, InteractiveElement, IntoElement, Modifiers, MouseDownEvent,
        ParentElement, Render, Role, Styled, Window, deferred, div, point, px,
    };
    use std::{cell::Cell, rc::Rc};

    #[derive(Clone, PartialEq, gpui::Action)]
    #[action(namespace = test, no_json)]
    struct MenuTestAction;

    struct MenuClickTestView {
        clicked: Rc<Cell<usize>>,
        dismissed: Rc<Cell<usize>>,
        action_received: Rc<Cell<usize>>,
        focus: FocusHandle,
    }

    impl Render for MenuClickTestView {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let clicked = self.clicked.clone();
            let dismissed = self.dismissed.clone();
            let action_received = self.action_received.clone();
            let palette = palette(window, cx);
            let row = menu_action_with_state(
                "test-menu-row",
                "Choose item",
                Role::MenuItem,
                false,
                false,
                Box::new(cx.listener(move |_, _, window, cx| {
                    clicked.set(clicked.get() + 1);
                    window.dispatch_action(Box::new(MenuTestAction), cx);
                })),
                palette,
            );
            let outside_click = move |_: &MouseDownEvent, _: &mut Window, cx: &mut gpui::App| {
                dismissed.set(dismissed.get() + 1);
                cx.stop_propagation();
            };

            div()
                .track_focus(&self.focus)
                .size_full()
                .on_action(cx.listener(move |_, _: &MenuTestAction, _, _cx| {
                    action_received.set(action_received.get() + 1);
                }))
                .child(
                    deferred(
                        div()
                            .absolute()
                            .left(px(20.0))
                            .top(px(20.0))
                            .w(px(180.0))
                            .occlude()
                            .on_mouse_down_out(outside_click)
                            .child(div().w(px(180.0)).child(row)),
                    )
                    .priority(8),
                )
        }
    }

    #[gpui::test]
    fn menu_rows_receive_clicks_without_dismissing_the_popover(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            super::super::super::settings::load_settings(
                None,
                photon_core::BrowserSettings::default(),
                None,
                cx,
            );
        });
        let clicked = Rc::new(Cell::new(0));
        let dismissed = Rc::new(Cell::new(0));
        let action_received = Rc::new(Cell::new(0));
        let (_view, cx) = cx.add_window_view({
            let clicked = clicked.clone();
            let dismissed = dismissed.clone();
            let action_received = action_received.clone();
            move |window, cx| {
                let view = MenuClickTestView {
                    clicked,
                    dismissed,
                    action_received,
                    focus: cx.focus_handle(),
                };
                window.focus(&view.focus, cx);
                view
            }
        });

        cx.simulate_click(point(px(40.0), px(30.0)), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(clicked.get(), 1);
        assert_eq!(action_received.get(), 1);
        assert_eq!(dismissed.get(), 0);

        cx.simulate_click(point(px(240.0), px(120.0)), Modifiers::none());
        assert_eq!(dismissed.get(), 1);
    }
}
