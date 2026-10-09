//! Actions dispatched by browser keyboard shortcuts.

gpui::actions!(
    photon,
    [
        /// Open a new tab and focus its address field.
        NewTab,
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
    ]
);
