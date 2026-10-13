//! The macOS menu bar and Dock menu. Their items dispatch the browser's
//! actions, so they show the same shortcuts and do what the shortcuts do;
//! the app-wide ones are handled here, without a window. The menu bar
//! follows the frontmost window: its checkmarks, and Reload or Stop.

use gpui::{App, Global, Menu, MenuItem, OsAction, SystemMenuType};
use gpui_elements::editable_text::actions::{Copy, Cut, Paste, Redo, SelectAll, Undo};
use photon_brand::NAME;
use photon_shortcuts::{
    CloseTab, FindInPage, FindNext, FindPrevious, GoBack, GoForward, HideApp, HideOtherApps,
    MinimizeWindow, NewWindow, NextSpace, OpenCommandBar, OpenSettings, PreviousSpace, Quit,
    Reload, ReopenClosedTab, ResetZoom, SelectNextTab, SelectPreviousTab, ShowAllApps,
    ShowKeyboardShortcuts, StopLoading, ToggleSidebar, ToggleSidebarIcons, ToggleZenMode, ZoomIn,
    ZoomOut, ZoomWindow,
};

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
    cx.set_dock_menu(vec![MenuItem::action("New Window", NewWindow)]);
}

pub(crate) fn update_menu_bar(state: MenuBarState, cx: &mut App) {
    show_menu_bar(state, cx);
}

fn menu_bar(state: MenuBarState) -> Vec<Menu> {
    let reload = if state.loading {
        MenuItem::action("Stop Loading", StopLoading)
    } else {
        MenuItem::action("Reload Page", Reload)
    };
    let mut view = Vec::new();
    if state.has_sidebar {
        view.extend([
            MenuItem::action("Show or Hide Sidebar", ToggleSidebar),
            MenuItem::action("Sidebar as Icons", ToggleSidebarIcons).checked(state.sidebar_icons),
        ]);
    }
    view.extend([
        MenuItem::action("Zen Mode", ToggleZenMode).checked(state.zen),
        MenuItem::separator(),
        MenuItem::action("Zoom In", ZoomIn),
        MenuItem::action("Zoom Out", ZoomOut),
        MenuItem::action("Actual Size", ResetZoom),
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
            MenuItem::action("New Tab…", OpenCommandBar),
            MenuItem::action("New Window", NewWindow),
            MenuItem::action("Reopen Closed Tab", ReopenClosedTab),
            MenuItem::separator(),
            MenuItem::action("Close Tab", CloseTab),
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
            MenuItem::action("Find in Page…", FindInPage),
            MenuItem::action("Find Next", FindNext),
            MenuItem::action("Find Previous", FindPrevious),
        ]),
        Menu::new("View").items(view),
        Menu::new("History").items([
            MenuItem::action("Back", GoBack),
            MenuItem::action("Forward", GoForward),
        ]),
        // AppKit lists the open windows at the end of the menu named
        // "Window".
        Menu::new("Window").items([
            MenuItem::action("Minimize", MinimizeWindow),
            MenuItem::action("Zoom", ZoomWindow),
            MenuItem::separator(),
            MenuItem::action("Next Tab", SelectNextTab),
            MenuItem::action("Previous Tab", SelectPreviousTab),
            MenuItem::action("Next Space", NextSpace),
            MenuItem::action("Previous Space", PreviousSpace),
        ]),
        Menu::new("Help").items([MenuItem::action(
            "Keyboard Shortcuts",
            ShowKeyboardShortcuts,
        )]),
    ]
}
