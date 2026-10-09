//! Shared UI measurements in logical pixels.
//!
//! Keep repeated geometry, spacing, and type sizes here so shell views stay
//! visually consistent and can be tuned without searching individual views.

pub(super) const TITLEBAR_HEIGHT: f32 = 36.0;
pub(in crate::platform) const INITIAL_WINDOW_WIDTH: f32 = 1200.0;
pub(in crate::platform) const INITIAL_WINDOW_HEIGHT: f32 = 760.0;
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

pub(super) const CRASH_ALERT_MAX_WIDTH: f32 = 360.0;
pub(super) const CRASH_ALERT_INSET: f32 = 16.0;
pub(super) const CRASH_ALERT_PADDING: f32 = 12.0;
pub(super) const CRASH_ALERT_GAP: f32 = 8.0;
pub(super) const CRASH_ALERT_TITLE_SIZE: f32 = 14.0;

pub(super) const OMNIBOX_HEIGHT: f32 = 28.0;
pub(super) const OMNIBOX_HORIZONTAL_PADDING: f32 = 10.0;
pub(super) const OMNIBOX_GAP: f32 = 8.0;
pub(super) const OMNIBOX_RADIUS: f32 = 8.0;
pub(super) const OMNIBOX_FONT_SIZE: f32 = 13.0;
pub(super) const OMNIBOX_ICON_SIZE: f32 = 13.0;

pub(super) const PAGE_INSET: f32 = 4.0;
pub(super) const WEBVIEW_CORNER_RADIUS: f32 = 12.0;
