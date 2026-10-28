//! Actions dispatched by browser keyboard shortcuts.

gpui::actions!(
    photon,
    [
        /// Open a new tab and focus its address field.
        NewTab,
        /// Open the browser command palette.
        OpenCommandPalette,
        /// Open a new browser window.
        NewWindow,
        /// Close the active tab, or the window with its last tab.
        CloseTab,
        /// Reopen the most recently closed tab in this window.
        ReopenClosedTab,
        /// Activate the tab to the right, wrapping to the first.
        SelectNextTab,
        /// Activate the tab to the left, wrapping to the last.
        SelectPreviousTab,
        /// Activate the first through eighth tabs.
        SelectTab1,
        SelectTab2,
        SelectTab3,
        SelectTab4,
        SelectTab5,
        SelectTab6,
        SelectTab7,
        SelectTab8,
        /// Activate the last tab, however many there are.
        SelectLastTab,
        /// Reload the active page.
        Reload,
        /// Stop loading the active page.
        StopLoading,
        /// Go back in the active tab's history.
        GoBack,
        /// Go forward in the active tab's history.
        GoForward,
        /// Move keyboard focus to the omnibox and select its contents.
        FocusOmnibox,
        /// Open the find bar, or focus it when it is open.
        FindInPage,
        /// Go to the next find-in-page match.
        FindNext,
        /// Go to the previous find-in-page match.
        FindPrevious,
        /// Zoom the page in a step.
        ZoomIn,
        /// Zoom the page out a step.
        ZoomOut,
        /// Return the page to 100%.
        ResetZoom,
        /// Show or hide the sidebar.
        ToggleSidebar,
        /// Collapse the sidebar to icons, or expand it again.
        ToggleSidebarIcons,
        /// Hide all browser chrome around the page, or bring it back.
        ToggleZenMode,
        /// Open the browser's settings in a tab.
        OpenSettings,
        /// Open the history page in a tab.
        OpenHistory,
        /// Open the downloads page in a tab.
        OpenDownloads,
        /// Open the Engine and browser task manager.
        OpenTaskManager,
        /// Show or hide the active page's performance details.
        ToggleDebugInfo,
        /// List the browser's keyboard shortcuts.
        ShowKeyboardShortcuts,
        /// Quit the browser.
        Quit,
        /// Hide the browser's windows.
        HideApp,
        /// Hide every other app's windows.
        HideOtherApps,
        /// Show every app's hidden windows.
        ShowAllApps,
        /// Minimize the window into the Dock.
        MinimizeWindow,
        /// Zoom the window to fill the screen, or back.
        ZoomWindow,
        /// Show the next space.
        NextSpace,
        /// Show the previous space.
        PreviousSpace,
    ]
);
