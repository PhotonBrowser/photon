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
/// Space reserved at each side of the titlebar for native window controls.
pub(super) const WINDOW_CONTROLS_INSET: f32 = 80.0;

pub(super) const TAB_HEIGHT: f32 = 28.0;
pub(super) const TAB_MIN_WIDTH: f32 = 72.0;
pub(super) const TAB_MAX_WIDTH: f32 = 220.0;
pub(super) const TAB_STRIP_GAP: f32 = 2.0;
pub(super) const TAB_STRIP_INSET: f32 = 4.0;
pub(super) const TAB_HORIZONTAL_PADDING: f32 = 10.0;
pub(super) const TAB_CLOSE_GAP: f32 = 6.0;
pub(super) const TAB_CLOSE_BUTTON_SIZE: f32 = 22.0;
pub(super) const TAB_ICON_SIZE: f32 = 12.0;
pub(super) const TAB_FAVICON_SIZE: f32 = 16.0;
pub(super) const TAB_FONT_SIZE: f32 = 12.0;
pub(super) const CONTROL_RADIUS: f32 = 7.0;

pub(super) const TOOLBAR_HEIGHT: f32 = 30.0;
/// Titlebar and address toolbar, laid out as one cached view.
pub(super) const CHROME_HEIGHT: f32 = TITLEBAR_HEIGHT + TOOLBAR_HEIGHT;
pub(super) const TOOLBAR_HORIZONTAL_INSET: f32 = 12.0;
pub(super) const TOOLBAR_CONTROL_GAP: f32 = 8.0;
pub(super) const TOOLBAR_BUTTON_SIZE: f32 = 28.0;
pub(super) const TOOLBAR_ICON_SIZE: f32 = 14.0;

pub(super) const MENU_WIDTH: f32 = 196.0;
pub(super) const MENU_PADDING: f32 = 4.0;
pub(super) const SURFACE_RADIUS: f32 = 8.0;
pub(super) const MENU_FONT_SIZE: f32 = 13.0;
pub(super) const MENU_ITEM_HEIGHT: f32 = 30.0;
pub(super) const MENU_ITEM_HORIZONTAL_PADDING: f32 = 8.0;
pub(super) const MENU_ITEM_RADIUS: f32 = 5.0;
pub(super) const MENU_ITEM_GAP: f32 = 2.0;
pub(super) const MENU_SECTION_INSET: f32 = 6.0;
pub(super) const MENU_SEPARATOR_HEIGHT: f32 = 1.0;

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

pub(super) const OMNIBOX_HEIGHT: f32 = 28.0;
pub(super) const OMNIBOX_HORIZONTAL_PADDING: f32 = 10.0;
pub(super) const OMNIBOX_GAP: f32 = 8.0;
pub(super) const OMNIBOX_RADIUS: f32 = 8.0;
pub(super) const OMNIBOX_FONT_SIZE: f32 = 13.0;
pub(super) const OMNIBOX_ICON_SIZE: f32 = 13.0;

pub(super) const PAGE_INSET: f32 = 4.0;
pub(super) const POPUP_URL_FONT_SIZE: f32 = 12.0;
pub(super) const WEBVIEW_CORNER_RADIUS: f32 = 12.0;
