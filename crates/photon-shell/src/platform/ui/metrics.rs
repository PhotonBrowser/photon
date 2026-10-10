//! Shared UI measurements in logical pixels.
//!
//! Keep repeated geometry, spacing, and type sizes here so shell views stay
//! visually consistent and can be tuned without searching individual views.

pub(super) const TITLEBAR_HEIGHT: f32 = 36.0;
pub(in crate::platform) const INITIAL_WINDOW_WIDTH: f32 = 1200.0;
pub(in crate::platform) const INITIAL_WINDOW_HEIGHT: f32 = 760.0;
pub(in crate::platform) const POPUP_WINDOW_WIDTH: f32 = 720.0;
pub(in crate::platform) const POPUP_WINDOW_HEIGHT: f32 = 620.0;
pub(in crate::platform) const POPUP_MIN_WINDOW_WIDTH: f32 = 360.0;
pub(in crate::platform) const POPUP_MAX_WINDOW_WIDTH: f32 = 1280.0;
pub(in crate::platform) const POPUP_MIN_WINDOW_HEIGHT: f32 = 320.0;
pub(in crate::platform) const POPUP_MAX_WINDOW_HEIGHT: f32 = 1000.0;
const WINDOW_CONTROLS_HEIGHT: f32 = 14.0;
/// Vertically centered origin of the native window control buttons.
pub(in crate::platform) const WINDOW_CONTROLS_ORIGIN: (f32, f32) =
    (12.0, (TITLEBAR_HEIGHT - WINDOW_CONTROLS_HEIGHT) / 2.0);
/// Space reserved at each side of the windowed titlebar for native controls.
pub(super) const WINDOW_CONTROLS_INSET: f32 = 80.0;

pub(super) const TAB_HEIGHT: f32 = 28.0;
pub(super) const TAB_MIN_WIDTH: f32 = 72.0;
/// A pinned tab in the strip, which shows only its icon.
pub(super) const PINNED_TAB_WIDTH: f32 = 36.0;
pub(super) const TAB_STRIP_GAP: f32 = 2.0;
pub(super) const TAB_STRIP_INSET: f32 = 4.0;
pub(super) const TAB_CLOSE_GAP: f32 = 6.0;
pub(super) const TAB_MAX_WIDTH: f32 = 220.0;
pub(super) const TAB_HORIZONTAL_PADDING: f32 = 10.0;
pub(super) const TAB_CLOSE_BUTTON_SIZE: f32 = 22.0;
pub(super) const TAB_ICON_SIZE: f32 = 12.0;
pub(super) const TAB_FAVICON_SIZE: f32 = 16.0;
/// A dragged tab's preview is slightly see-through, showing where it lands.
pub(super) const TAB_DRAG_PREVIEW_OPACITY: f32 = 0.9;
pub(super) const TAB_AUDIO_BUTTON_SIZE: f32 = 22.0;
pub(super) const TAB_AUDIO_ICON_SIZE: f32 = TAB_FAVICON_SIZE;
pub(super) const TAB_FONT_SIZE: f32 = 12.0;
pub(super) const CONTROL_RADIUS: f32 = 7.0;

pub(super) const TOOLBAR_HEIGHT: f32 = 30.0;
/// The tab strip and address toolbar, laid out as one cached view.
pub(super) const CHROME_HEIGHT: f32 = TITLEBAR_HEIGHT + TOOLBAR_HEIGHT;
pub(super) const TOOLBAR_HORIZONTAL_INSET: f32 = 12.0;
pub(super) const TOOLBAR_CONTROL_GAP: f32 = 8.0;
pub(super) const TOOLBAR_BUTTON_SIZE: f32 = 28.0;
pub(super) const TOOLBAR_ICON_SIZE: f32 = 14.0;

pub(super) const MENU_WIDTH: f32 = 220.0;
/// Space above the first and below the last menu row.
pub(super) const MENU_PADDING: f32 = 4.0;
pub(super) const SURFACE_RADIUS: f32 = 8.0;
pub(super) const MENU_FONT_SIZE: f32 = 13.0;
pub(super) const MENU_ITEM_HEIGHT: f32 = 28.0;
pub(super) const MENU_ITEM_HORIZONTAL_PADDING: f32 = 12.0;
pub(super) const MENU_ITEM_RADIUS: f32 = 5.0;
pub(super) const MENU_ITEM_GAP: f32 = 2.0;
/// Room for "100%" between a menu stepper's buttons.
pub(super) const MENU_STEPPER_VALUE_WIDTH: f32 = 48.0;
pub(super) const MENU_SEPARATOR_HEIGHT: f32 = 1.0;
/// Space above and below a menu separator.
pub(super) const MENU_SEPARATOR_MARGIN: f32 = 4.0;

pub(super) const FIND_BAR_HEIGHT: f32 = 34.0;
pub(super) const FIND_BAR_PADDING: f32 = 10.0;
pub(super) const FIND_BAR_TRAILING_PADDING: f32 = 5.0;
pub(super) const FIND_BAR_GAP: f32 = 6.0;
pub(super) const FIND_FIELD_WIDTH: f32 = 180.0;
pub(super) const FIND_FONT_SIZE: f32 = 13.0;
pub(super) const FIND_RESULT_FONT_SIZE: f32 = 12.0;

/// Distance from the page's corner to a status chip.
pub(super) const CHIP_INSET: f32 = 10.0;
pub(super) const CHIP_HEIGHT: f32 = 26.0;
pub(super) const CHIP_PADDING: f32 = 10.0;
/// Right padding beside a chip's buttons, which bring their own.
pub(super) const CHIP_TRAILING_PADDING: f32 = 3.0;
pub(super) const CHIP_GAP: f32 = 6.0;
pub(super) const CHIP_ICON_SIZE: f32 = 12.0;
pub(super) const CHIP_FONT_SIZE: f32 = 12.0;
pub(super) const CHIP_CLOSE_SIZE: f32 = 20.0;
pub(super) const CHIP_CLOSE_ICON_SIZE: f32 = 10.0;

pub(super) const BUTTON_HEIGHT: f32 = 30.0;
pub(super) const BUTTON_MIN_WIDTH: f32 = 76.0;
pub(super) const BUTTON_HORIZONTAL_PADDING: f32 = 16.0;
pub(super) const BUTTON_FONT_SIZE: f32 = 13.0;
pub(super) const BUTTON_GAP: f32 = 8.0;
pub(super) const ICON_BUTTON_SIZE: f32 = 24.0;
pub(super) const ICON_BUTTON_ICON_SIZE: f32 = 13.0;
/// Opacity of a control that cannot be used right now.
pub(super) const DISABLED_OPACITY: f32 = 0.4;
pub(super) const SMALL_BUTTON_HEIGHT: f32 = 20.0;
pub(super) const SMALL_BUTTON_HORIZONTAL_PADDING: f32 = 9.0;
pub(super) const SMALL_BUTTON_FONT_SIZE: f32 = 12.0;

pub(super) const MODAL_WIDTH: f32 = 400.0;
pub(super) const MODAL_PADDING: f32 = 18.0;
pub(super) const MODAL_GAP: f32 = 12.0;
pub(super) const MODAL_SURFACE_INSET: f32 = 16.0;
pub(super) const DIALOG_MESSAGE_MAX_HEIGHT: f32 = 240.0;

/// The address field in the toolbar.
pub(super) const OMNIBOX_HEIGHT: f32 = 28.0;
/// The larger address field in the sidebar.
pub(super) const SIDEBAR_OMNIBOX_HEIGHT: f32 = 42.0;
pub(super) const SIDEBAR_OMNIBOX_RADIUS: f32 = 12.0;
pub(super) const SIDEBAR_OMNIBOX_FONT_SIZE: f32 = 15.0;
pub(super) const OMNIBOX_BORDER_WIDTH: f32 = 1.0;
pub(super) const OMNIBOX_HORIZONTAL_PADDING: f32 = 12.0;
pub(super) const OMNIBOX_GAP: f32 = 8.0;
pub(super) const OMNIBOX_RADIUS: f32 = 8.0;
pub(super) const OMNIBOX_FONT_SIZE: f32 = 13.0;
pub(super) const OMNIBOX_ICON_SIZE: f32 = 13.0;
/// Space around the suggestion rows in the open omnibox.
pub(super) const OMNIBOX_SUGGESTIONS_INSET: f32 = 4.0;
pub(super) const OMNIBOX_SUGGESTION_HEIGHT: f32 = 28.0;
pub(super) const OMNIBOX_SUGGESTION_ICON_SIZE: f32 = 16.0;
/// The open omnibox reaches over the page when the field is narrower.
pub(super) const OMNIBOX_PANEL_MIN_WIDTH: f32 = 520.0;

pub(super) const PAGE_INSET: f32 = 8.0;
/// A smaller gap between horizontal browser chrome and the page surface.
pub(super) const PAGE_CHROME_GAP: f32 = 4.0;
pub(super) const POPUP_URL_FONT_SIZE: f32 = 12.0;
/// The corners of the frame a page sits in, web page or the browser's own.
pub(super) const PAGE_RADIUS: f32 = 12.0;
pub(super) const INTERNAL_PAGE_INSET: f32 = 32.0;
pub(super) const INTERNAL_PAGE_MAX_WIDTH: f32 = 620.0;
pub(super) const INTERNAL_PAGE_SECTION_GAP: f32 = 28.0;
pub(super) const INTERNAL_PAGE_GROUP_GAP: f32 = 12.0;
pub(super) const INTERNAL_PAGE_SECTION_SIZE: f32 = 18.0;
pub(super) const INTERNAL_PAGE_BODY_SIZE: f32 = 14.0;
pub(super) const SWITCH_WIDTH: f32 = 34.0;
pub(super) const SWITCH_KNOB_SIZE: f32 = 16.0;
pub(super) const SWITCH_INSET: f32 = 2.0;
pub(super) const CHECKBOX_SIZE: f32 = 16.0;
pub(super) const CHECKBOX_RADIUS: f32 = 4.0;
pub(super) const CHECKBOX_ICON_SIZE: f32 = 12.0;
pub(super) const NEW_TAB_LOGO_SIZE: f32 = 64.0;
pub(super) const SHORTCUT_TILE_WIDTH: f32 = 104.0;
pub(super) const SHORTCUT_ICON_BOX_SIZE: f32 = 48.0;
pub(super) const SHORTCUT_ICON_SIZE: f32 = 24.0;
pub(super) const SHORTCUT_GAP: f32 = 8.0;
pub(super) const SHORTCUT_REMOVE_INSET: f32 = 4.0;
pub(super) const SEGMENT_HEIGHT: f32 = 26.0;
pub(super) const DROPDOWN_WIDTH: f32 = 160.0;
/// A colour swatch, and the ring around the chosen one.
pub(super) const SWATCH_SIZE: f32 = 22.0;
pub(super) const SWATCH_RING_SIZE: f32 = 30.0;
pub(super) const SWATCH_GAP: f32 = 6.0;
pub(super) const SEGMENT_HORIZONTAL_PADDING: f32 = 12.0;
pub(super) const SEGMENT_RADIUS: f32 = 5.0;
pub(super) const SEGMENT_GAP: f32 = 2.0;
pub(super) const SEGMENT_INSET: f32 = 2.0;
pub(super) const SETTINGS_ROW_HEIGHT: f32 = 34.0;
pub(super) const SETTINGS_SIDEBAR_WIDTH: f32 = 200.0;
pub(super) const SETTINGS_SIDEBAR_ITEM_HEIGHT: f32 = 30.0;
pub(super) const SETTINGS_SIDEBAR_ICON_SIZE: f32 = 14.0;

// The sidebar: navigation, address field, favourites, tabs and its footer.
/// The grab area on the sidebar's edge for resizing it.
pub(super) const SIDEBAR_RESIZE_HANDLE_WIDTH: f32 = 8.0;
/// Space between the sidebar's edges and its content.
pub(super) const SIDEBAR_PADDING: f32 = 8.0;
/// Space between the sidebar's top-row buttons.
pub(super) const SIDEBAR_CONTROL_GAP: f32 = 8.0;
/// Space between the sidebar's sections.
pub(super) const SIDEBAR_SECTION_GAP: f32 = 8.0;
pub(super) const SIDEBAR_ITEM_RADIUS: f32 = 12.0;
/// Space between tab rows, and between favourite tiles.
pub(super) const SIDEBAR_ITEM_GAP: f32 = 6.0;
pub(super) const SIDEBAR_FAVOURITE_GAP: f32 = 8.0;
pub(super) const SIDEBAR_TAB_HEIGHT: f32 = 40.0;
pub(super) const SIDEBAR_TAB_PADDING: f32 = 12.0;
/// Space between a tab's icon and its title.
pub(super) const SIDEBAR_TAB_ICON_GAP: f32 = 10.0;
pub(super) const SIDEBAR_FONT_SIZE: f32 = 14.0;
pub(super) const SIDEBAR_ICON_SIZE: f32 = 18.0;
pub(super) const SIDEBAR_FAVOURITE_COLUMNS: usize = 3;
pub(super) const SIDEBAR_FAVOURITE_HEIGHT: f32 = 50.0;
pub(super) const SIDEBAR_FAVOURITE_ICON_SIZE: f32 = 20.0;
pub(super) const SIDEBAR_FOOTER_HEIGHT: f32 = 40.0;
pub(super) const SIDEBAR_SEPARATOR_MARGIN: f32 = 8.0;
/// The spaces in the sidebar's footer: a dot each, larger while shown.
pub(super) const SPACE_DOT_SIZE: f32 = 7.0;
pub(super) const SPACE_DOT_ACTIVE_SIZE: f32 = 10.0;
pub(super) const SPACE_DOT_BUTTON_SIZE: f32 = 22.0;
pub(super) const SPACE_DOT_GAP: f32 = 2.0;
pub(super) const SPACE_DOT_IDLE_OPACITY: f32 = 0.7;

/// The command bar ⌘T opens over the page.
pub(super) const COMMAND_BAR_WIDTH: f32 = 640.0;
/// Space above the command bar, from the top of the window.
pub(super) const COMMAND_BAR_TOP: f32 = 120.0;
pub(super) const COMMAND_BAR_RADIUS: f32 = 14.0;
pub(super) const COMMAND_BAR_FIELD_HEIGHT: f32 = 52.0;
pub(super) const COMMAND_BAR_FIELD_PADDING: f32 = 16.0;
pub(super) const COMMAND_BAR_FONT_SIZE: f32 = 18.0;
pub(super) const COMMAND_BAR_ICON_SIZE: f32 = 18.0;
pub(super) const COMMAND_BAR_ROW_HEIGHT: f32 = 40.0;
pub(super) const COMMAND_BAR_ROWS_INSET: f32 = 6.0;
pub(super) const COMMAND_BAR_DETAIL_FONT_SIZE: f32 = 12.0;
