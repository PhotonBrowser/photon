//! The macOS menu bar and Dock menu. Their items dispatch the browser's
//! actions, so they show the same shortcuts and do what the shortcuts do;
//! the app-wide ones are handled here, without a window. The menu bar
//! follows the frontmost window: its checkmarks, and Reload or Stop.

use crate::platform::shortcuts::{
    CloseTab, FindInPage, FindNext, FindPrevious, GoBack, GoForward, HideApp, HideOtherApps,
    MinimizeWindow, NewTab, NewWindow, NextSpace, OpenCommandPalette, OpenDownloads, OpenHistory,
    OpenSettings, OpenTaskManager, PreviousSpace, Quit, Reload, ReopenClosedTab, ResetZoom,
    SelectNextTab, SelectPreviousTab, ShowAllApps, ShowKeyboardShortcuts, StopLoading,
    ToggleSidebar, ToggleSidebarIcons, ToggleZenMode, ZoomIn, ZoomOut, ZoomWindow,
    shortcut_name_for_action,
};
use gpui::{App, Global, Menu, MenuItem, OsAction, SystemMenuType};
use gpui_elements::editable_text::actions::{Copy, Cut, Paste, Redo, SelectAll, Undo};
use photon_brand::NAME;
use photon_core::Command;

use crate::platform::MenuBarState;

/// The state the menu bar was last built for.
struct ShownMenuBar(MenuBarState);

impl Global for ShownMenuBar {}

/// Builds the menu bar for `state`, unless it already shows it.
fn show_menu_bar(state: MenuBarState, cx: &mut App) {
    if cx
        .try_global::<ShownMenuBar>()
        .is_some_and(|shown| shown.0 == state)
    {
        return;
    }
    cx.set_global(ShownMenuBar(state));
    cx.set_menus(menu_bar(state));
}

/// Installs the menu bar and Dock menu, and handles the actions that need
/// no window: quitting, hiding, and opening a window, which the Dock menu
/// asks for even while no window is active.
pub(crate) fn install_app_menus(cx: &mut App) {
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.on_action(|_: &HideApp, cx| cx.hide());
    cx.on_action(|_: &HideOtherApps, cx| cx.hide_other_apps());
    cx.on_action(|_: &ShowAllApps, cx| cx.unhide_other_apps());
    show_menu_bar(MenuBarState::default(), cx);
    cx.set_dock_menu(vec![MenuItem::action(label(&NewWindow), NewWindow)]);
}

pub(crate) fn update_menu_bar(state: MenuBarState, cx: &mut App) {
    show_menu_bar(state, cx);
}

fn menu_bar(state: MenuBarState) -> Vec<Menu> {
    let commands = state.commands;
    let reload = if state.loading {
        MenuItem::action(label(&StopLoading), StopLoading)
    } else {
        MenuItem::action(label(&Reload), Reload)
    }
    .disabled(!commands.allows(Command::ReloadPage));
    let mut view = vec![
        MenuItem::action(label(&OpenCommandPalette), OpenCommandPalette),
        MenuItem::separator(),
    ];
    if commands.has_sidebar {
        view.extend([
            MenuItem::action(label(&ToggleSidebar), ToggleSidebar),
            MenuItem::action(label(&ToggleSidebarIcons), ToggleSidebarIcons)
                .checked(state.sidebar_icons),
        ]);
    }
    view.extend([
        MenuItem::action(label(&ToggleZenMode), ToggleZenMode).checked(state.zen),
        MenuItem::separator(),
        MenuItem::action(label(&ZoomIn), ZoomIn),
        MenuItem::action(label(&ZoomOut), ZoomOut),
        MenuItem::action(label(&ResetZoom), ResetZoom),
        MenuItem::separator(),
        MenuItem::action(label(&OpenHistory), OpenHistory),
        MenuItem::action(label(&OpenDownloads), OpenDownloads),
        MenuItem::action(label(&OpenTaskManager), OpenTaskManager),
        MenuItem::separator(),
        reload,
    ]);
    vec![
        Menu::new(NAME).items([
            MenuItem::action("Settings…", OpenSettings),
            MenuItem::separator(),
            MenuItem::os_submenu("Services", SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action(format!("Hide {NAME}"), HideApp),
            MenuItem::action("Hide Others", HideOtherApps),
            MenuItem::action("Show All", ShowAllApps),
            MenuItem::separator(),
            MenuItem::action(format!("Quit {NAME}"), Quit),
        ]),
        Menu::new("File").items([
            MenuItem::action(label(&NewTab), NewTab),
            MenuItem::action(label(&NewWindow), NewWindow),
            MenuItem::action(label(&ReopenClosedTab), ReopenClosedTab)
                .disabled(!commands.allows(Command::ReopenClosedTab)),
            MenuItem::separator(),
            MenuItem::action(label(&CloseTab), CloseTab),
        ]),
        Menu::new("Edit").items([
            MenuItem::os_action("Undo", Undo, OsAction::Undo),
            MenuItem::os_action("Redo", Redo, OsAction::Redo),
            MenuItem::separator(),
            MenuItem::os_action("Cut", Cut, OsAction::Cut),
            MenuItem::os_action("Copy", Copy, OsAction::Copy),
            MenuItem::os_action("Paste", Paste, OsAction::Paste),
            MenuItem::os_action("Select All", SelectAll, OsAction::SelectAll),
            MenuItem::separator(),
            MenuItem::action(label(&FindInPage), FindInPage)
                .disabled(!commands.allows(Command::FindInPage)),
            MenuItem::action(label(&FindNext), FindNext).disabled(!commands.has_page),
            MenuItem::action(label(&FindPrevious), FindPrevious).disabled(!commands.has_page),
        ]),
        Menu::new("View").items(view),
        Menu::new("History").items([
            MenuItem::action(label(&GoBack), GoBack),
            MenuItem::action(label(&GoForward), GoForward),
        ]),
        // AppKit lists the open windows at the end of the menu named
        // "Window".
        Menu::new("Window").items([
            MenuItem::action(label(&MinimizeWindow), MinimizeWindow),
            MenuItem::action("Zoom", ZoomWindow),
            MenuItem::separator(),
            MenuItem::action(label(&SelectNextTab), SelectNextTab),
            MenuItem::action(label(&SelectPreviousTab), SelectPreviousTab),
            MenuItem::action(label(&NextSpace), NextSpace)
                .disabled(!commands.allows(Command::NextSpace)),
            MenuItem::action(label(&PreviousSpace), PreviousSpace)
                .disabled(!commands.allows(Command::PreviousSpace)),
        ]),
        Menu::new("Help").items([MenuItem::action(
            label(&ShowKeyboardShortcuts),
            ShowKeyboardShortcuts,
        )]),
    ]
}

fn label(action: &dyn gpui::Action) -> &'static str {
    shortcut_name_for_action(action).expect("native menu action has a shortcut registry entry")
}

#[cfg(test)]
mod tests {
    use super::*;
    use photon_core::CommandAvailability;

    fn menu_item<'a>(menus: &'a [Menu], menu_name: &str, item_name: &str) -> &'a MenuItem {
        menus
            .iter()
            .find(|menu| menu.name.as_ref() == menu_name)
            .and_then(|menu| {
                menu.items.iter().find(|item| {
                    matches!(item, MenuItem::Action { name, .. } if name.as_ref() == item_name)
                })
            })
            .unwrap_or_else(|| panic!("missing {item_name:?} item in {menu_name:?} menu"))
    }

    #[test]
    fn native_menu_commands_follow_current_availability() {
        let unavailable = menu_bar(MenuBarState::default());
        assert!(menu_item(&unavailable, "File", "Reopen Closed Tab").is_disabled());
        assert!(menu_item(&unavailable, "Edit", "Find in Page").is_disabled());
        assert!(menu_item(&unavailable, "Window", "Next Space").is_disabled());

        let available = menu_bar(MenuBarState {
            commands: CommandAvailability {
                has_page: true,
                has_multiple_tabs: true,
                has_closed_tabs: true,
                can_go_back: true,
                can_go_forward: true,
                is_loading: true,
                has_sidebar: true,
                shows_spaces: true,
                has_next_space: true,
                has_previous_space: true,
            },
            ..MenuBarState::default()
        });
        assert!(!menu_item(&available, "File", "Reopen Closed Tab").is_disabled());
        assert!(!menu_item(&available, "Edit", "Find in Page").is_disabled());
        assert!(!menu_item(&available, "Window", "Next Space").is_disabled());
    }
}
