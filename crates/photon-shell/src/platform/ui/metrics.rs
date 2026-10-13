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

pub(super) const TAB_HEIGHT: f32 = 28.0;
pub(super) const TAB_MIN_WIDTH: f32 = 72.0;
/// A pinned tab in the strip, which shows only its icon.
pub(super) const PINNED_TAB_WIDTH: f32 = 36.0;
pub(super) const TAB_STRIP_GAP: f32 = 2.0;
pub(super) const TAB_STRIP_INSET: f32 = 4.0;
pub(super) const TAB_CLOSE_GAP: f32 = 6.0;
/// How far a pointer must pass a tab container's detachable edge before it opens a window.
pub(super) const TAB_DETACH_DISTANCE: f32 = 40.0;
/// Pointer travel before a press becomes a tab drag.
pub(super) const TAB_DRAG_THRESHOLD: f32 = 4.0;
/// Edge zone and maximum speed for scrolling a tab strip during a drag.
pub(super) const TAB_DRAG_SCROLL_EDGE: f32 = 28.0;
pub(super) const TAB_DRAG_SCROLL_SPEED: f32 = 720.0;
pub(super) const TAB_DRAG_SOURCE_OPACITY: f32 = 0.28;
pub(super) const SLEEPING_TAB_OPACITY: f32 = 0.7;
pub(super) const TAB_REORDER_DURATION_MS: u64 = 150;
pub(super) const TAB_DROP_SETTLE_DURATION_MS: u64 = 120;
pub(super) const TAB_ENTRY_OFFSET: f32 = 5.0;
pub(super) const TAB_MAX_WIDTH: f32 = 220.0;
pub(super) const TAB_HORIZONTAL_PADDING: f32 = 10.0;
pub(super) const TAB_CLOSE_BUTTON_SIZE: f32 = 22.0;
pub(super) const TAB_ICON_SIZE: f32 = 12.0;
pub(super) const TAB_FAVICON_SIZE: f32 = 16.0;
pub(super) const TAB_AUDIO_BUTTON_SIZE: f32 = 22.0;
pub(super) const TAB_AUDIO_ICON_SIZE: f32 = TAB_FAVICON_SIZE;
pub(super) const TAB_FONT_SIZE: f32 = 12.0;
pub(super) const CONTROL_RADIUS: f32 = 7.0;

pub(super) const TOOLBAR_HEIGHT: f32 = 30.0;
pub(super) const TOOLBAR_HORIZONTAL_INSET: f32 = 12.0;
pub(super) const TOOLBAR_CONTROL_GAP: f32 = 8.0;
pub(super) const TOOLBAR_BUTTON_SIZE: f32 = 28.0;
pub(super) const TOOLBAR_ICON_SIZE: f32 = 14.0;
pub(super) const WINDOW_CONTROL_GAP: f32 = 2.0;
pub(super) const WINDOW_CONTROL_EDGE_INSET: f32 = 8.0;
#[cfg_attr(target_os = "macos", allow(dead_code))]
pub(super) const WINDOW_CONTROL_GROUP_WIDTH: f32 =
    TOOLBAR_BUTTON_SIZE * 3.0 + WINDOW_CONTROL_GAP * 2.0;

pub(super) const MENU_WIDTH: f32 = 220.0;

/// A keycap in a keyboard shortcut.
pub(super) const KBD_HEIGHT: f32 = 18.0;
pub(super) const KBD_PADDING: f32 = 4.0;
pub(super) const KBD_RADIUS: f32 = 4.0;
pub(super) const KBD_FONT_SIZE: f32 = 11.0;
/// Space between a shortcut's keycaps.
pub(super) const KBD_GAP: f32 = 2.0;
/// Space above the first and below the last menu row.
pub(super) const MENU_PADDING: f32 = 4.0;
pub(super) const SURFACE_RADIUS: f32 = 8.0;
pub(super) const MENU_FONT_SIZE: f32 = 13.0;
pub(super) const MENU_ITEM_HEIGHT: f32 = 28.0;
pub(super) const MENU_ITEM_HORIZONTAL_PADDING: f32 = 12.0;
pub(super) const MENU_ITEM_RADIUS: f32 = 5.0;
pub(super) const MENU_ITEM_GAP: f32 = 2.0;
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
pub(in crate::platform) const CHIP_INSET: f32 = 10.0;
pub(super) const CHIP_HEIGHT: f32 = 26.0;
/// A status chip's outer radius: its action radius plus the vertical inset.
pub(super) const CHIP_RADIUS: f32 = SMALL_BUTTON_RADIUS + (CHIP_HEIGHT - SMALL_BUTTON_HEIGHT) / 2.0;
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
/// Small buttons use a pill radius, matching half their height.
pub(super) const SMALL_BUTTON_RADIUS: f32 = SMALL_BUTTON_HEIGHT / 2.0;
pub(super) const SMALL_BUTTON_HORIZONTAL_PADDING: f32 = 9.0;
pub(super) const SMALL_BUTTON_FONT_SIZE: f32 = 12.0;

pub(super) const TOOLTIP_FONT_SIZE: f32 = 12.0;
pub(super) const TOOLTIP_HORIZONTAL_PADDING: f32 = 10.0;
pub(super) const TOOLTIP_VERTICAL_PADDING: f32 = 6.0;
pub(super) const TOOLTIP_RADIUS: f32 = 7.0;

pub(super) const MODAL_WIDTH: f32 = 400.0;
pub(super) const MODAL_PADDING: f32 = 18.0;
pub(super) const MODAL_GAP: f32 = 12.0;
pub(super) const MODAL_SURFACE_INSET: f32 = 16.0;
pub(super) const DIALOG_MESSAGE_MAX_HEIGHT: f32 = 240.0;
/// Every dialog's title, and the text under it.
pub(super) const DIALOG_TITLE_SIZE: f32 = 15.0;
pub(super) const DIALOG_TEXT_SIZE: f32 = 13.0;
/// The keyboard shortcuts dialog, wider than a modal's default for its
/// names and keys, and scrolling past this height.
pub(super) const SHORTCUTS_DIALOG_WIDTH: f32 = 460.0;
pub(super) const SHORTCUTS_DIALOG_MAX_HEIGHT: f32 = 520.0;

/// The address field in the toolbar.
pub(super) const OMNIBOX_HEIGHT: f32 = 28.0;
/// The larger address field in the sidebar.
pub(super) const SIDEBAR_OMNIBOX_HEIGHT: f32 = 38.0;
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
pub(in crate::platform) const PAGE_RADIUS: f32 = 12.0;
/// Horizontal travel required before a history swipe commits.
pub(super) const HISTORY_SWIPE_COMMIT_FRACTION: f32 = 0.33;
/// Minimum progress needed for a fast flick to commit a history swipe.
pub(super) const HISTORY_SWIPE_FLICK_FRACTION: f32 = 0.12;
/// Outward trackpad velocity that qualifies as a fast history swipe, in px/s.
pub(super) const HISTORY_SWIPE_FLICK_VELOCITY: f32 = 900.0;
/// Diameter of the arrow shown at the page edge during a history swipe.
pub(super) const HISTORY_SWIPE_INDICATOR_SIZE: f32 = 42.0;
pub(super) const HISTORY_SWIPE_INDICATOR_ICON_SIZE: f32 = 18.0;
/// Gesture travel at which the edge arrow becomes fully visible.
pub(super) const HISTORY_SWIPE_INDICATOR_REVEAL_DISTANCE: f32 = 20.0;
pub(super) const INTERNAL_PAGE_INSET: f32 = 32.0;
pub(super) const INTERNAL_PAGE_MAX_WIDTH: f32 = 620.0;
pub(super) const INTERNAL_PAGE_SECTION_GAP: f32 = 28.0;
pub(super) const INTERNAL_PAGE_GROUP_GAP: f32 = 12.0;
pub(super) const FORM_FIELD_GAP: f32 = 4.0;
pub(super) const INTERNAL_PAGE_SECTION_SIZE: f32 = 18.0;
pub(super) const INTERNAL_PAGE_BODY_SIZE: f32 = 14.0;
pub(super) const PERFORMANCE_PAGE_INSET: f32 = 32.0;
pub(super) const PERFORMANCE_PAGE_MAX_WIDTH: f32 = 1040.0;
pub(super) const PERFORMANCE_PAGE_SECTION_GAP: f32 = 24.0;
pub(super) const PERFORMANCE_PAGE_GAP: f32 = 16.0;
pub(super) const PERFORMANCE_CARD_PADDING: f32 = 14.0;
pub(super) const PERFORMANCE_TASK_GAP: f32 = 8.0;
pub(super) const PERFORMANCE_CHART_HEIGHT: f32 = 112.0;
pub(super) const PERFORMANCE_EVENT_LOOP_DELAY_THRESHOLD_MS: u64 = 50;
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
pub(super) const INTENSITY_SLIDER_HEIGHT: f32 = 26.0;
pub(super) const INTENSITY_SLIDER_THUMB_SIZE: f32 = 14.0;
pub(super) const INTENSITY_SLIDER_TRACK_HEIGHT: f32 = 4.0;
pub(super) const INTENSITY_SLIDER_VALUE_WIDTH: f32 = 40.0;
pub(super) const SETTINGS_ROW_HEIGHT: f32 = 34.0;
pub(super) const SETTINGS_SIDEBAR_WIDTH: f32 = 200.0;
pub(super) const SETTINGS_SIDEBAR_ITEM_HEIGHT: f32 = 30.0;
pub(super) const SETTINGS_SIDEBAR_ICON_SIZE: f32 = 14.0;

// The sidebar: navigation, address field, favourites, tabs and its footer.
/// Width of the hit area centred on the sidebar's resize edge.
pub(super) const SIDEBAR_RESIZE_HANDLE_WIDTH: f32 = 36.0;
/// The sidebar collapsed to a column of icons.
pub(super) const SIDEBAR_COLLAPSED_WIDTH: f32 = photon_core::SIDEBAR_COLLAPSED_WIDTH as f32;
/// Space between the collapsed sidebar's edges and its icons.
pub(super) const SIDEBAR_COLLAPSED_PADDING: f32 = 6.0;
/// A tab, favourite or button in the collapsed sidebar: a square that fills
/// it between its padding.
pub(super) const SIDEBAR_COLLAPSED_ITEM_SIZE: f32 =
    SIDEBAR_COLLAPSED_WIDTH - SIDEBAR_COLLAPSED_PADDING * 2.0;
pub(super) const SIDEBAR_COLLAPSED_ITEM_RADIUS: f32 = 8.0;
pub(super) const SIDEBAR_COLLAPSED_ICON_SIZE: f32 = 16.0;
/// Space between the collapsed sidebar's icons, and around its dividers.
pub(super) const SIDEBAR_COLLAPSED_GAP: f32 = 4.0;
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
pub(super) const SIDEBAR_TAB_HEIGHT: f32 = 36.0;
pub(super) const SIDEBAR_TAB_PADDING: f32 = 12.0;
/// Space between a tab's icon and its title.
pub(super) const SIDEBAR_TAB_ICON_GAP: f32 = 10.0;
pub(super) const SIDEBAR_FONT_SIZE: f32 = 14.0;
pub(super) const SIDEBAR_ICON_SIZE: f32 = 18.0;
pub(super) const SIDEBAR_FAVOURITE_COLUMNS: usize = 3;

/// The sidebar's spacing and icon size, which tighten when it collapses to
/// icons.
#[derive(Clone, Copy)]
pub(super) struct SidebarSpacing {
    /// Between its edges and its content.
    pub(super) padding: f32,
    /// Between its sections.
    pub(super) section_gap: f32,
    /// Above and below a divider.
    pub(super) separator_margin: f32,
    /// Between tabs.
    pub(super) item_gap: f32,
    /// Between favourites.
    pub(super) favourite_gap: f32,
    /// A tab's or favourite's icon.
    pub(super) icon_size: f32,
}

impl SidebarSpacing {
    pub(super) const fn new(collapsed: bool) -> Self {
        if collapsed {
            Self {
                padding: SIDEBAR_COLLAPSED_PADDING,
                section_gap: SIDEBAR_COLLAPSED_GAP,
                separator_margin: SIDEBAR_COLLAPSED_GAP,
                item_gap: SIDEBAR_COLLAPSED_GAP,
                favourite_gap: SIDEBAR_COLLAPSED_GAP,
                icon_size: SIDEBAR_COLLAPSED_ICON_SIZE,
            }
        } else {
            Self {
                padding: SIDEBAR_PADDING,
                section_gap: SIDEBAR_SECTION_GAP,
                separator_margin: SIDEBAR_SEPARATOR_MARGIN,
                item_gap: SIDEBAR_ITEM_GAP,
                favourite_gap: SIDEBAR_FAVOURITE_GAP,
                icon_size: SIDEBAR_ICON_SIZE,
            }
        }
    }
}
pub(super) const SIDEBAR_FAVOURITE_HEIGHT: f32 = 50.0;
pub(super) const SIDEBAR_FAVOURITE_ICON_SIZE: f32 = 20.0;
pub(super) const SIDEBAR_FAVOURITES_HINT_PADDING: f32 = 10.0;
pub(super) const SIDEBAR_FAVOURITES_HINT_GAP: f32 = 4.0;
pub(super) const SIDEBAR_FAVOURITES_HINT_BADGE_SIZE: f32 = 26.0;
pub(super) const SIDEBAR_FAVOURITES_HINT_ICON_SIZE: f32 = 14.0;
pub(super) const SIDEBAR_FAVOURITES_HINT_TITLE_SIZE: f32 = 15.0;
pub(super) const SIDEBAR_FAVOURITES_HINT_BODY_SIZE: f32 = 12.0;
pub(super) const SIDEBAR_FAVOURITES_HINT_DISMISS_INSET: f32 = 2.0;
pub(super) const SIDEBAR_FAVOURITES_HINT_DASH_LENGTH: f32 = 6.0;
pub(super) const SIDEBAR_FAVOURITES_HINT_DASH_GAP: f32 = 2.0;
pub(super) const SIDEBAR_FOOTER_HEIGHT: f32 = 40.0;
pub(super) const SIDEBAR_SEPARATOR_MARGIN: f32 = 8.0;
pub(super) const SIDEBAR_SEPARATOR_INSET: f32 = 10.0;
/// Gap around a sidebar temporarily revealed over the page.
pub(super) const SIDEBAR_REVEAL_INSET: f32 = 8.0;
pub(super) const SIDEBAR_REVEAL_RADIUS: f32 = PAGE_RADIUS;
/// The spaces in the sidebar's footer: a dot each, larger while shown.
pub(super) const SPACE_DOT_SIZE: f32 = 7.0;
pub(super) const SPACE_DOT_ACTIVE_SIZE: f32 = 10.0;
pub(super) const SPACE_DOT_BUTTON_SIZE: f32 = 22.0;
pub(super) const SPACE_DOT_GAP: f32 = 2.0;
pub(super) const SPACE_DOT_IDLE_OPACITY: f32 = 0.7;

/// Peek: the space around the preview, as a fraction of the window, and
/// between its buttons and the page.
pub(super) const PEEK_INSET_FRACTION: f32 = 0.08;
pub(super) const PEEK_CONTROL_GAP: f32 = 6.0;

/// The command bar ⌘T opens over the page.
pub(super) const COMMAND_BAR_WIDTH: f32 = 640.0;
/// How far down the window the command bar's field starts, as a fraction of
/// its height: a little above the middle.
pub(super) const COMMAND_BAR_TOP_FRACTION: f32 = 0.36;
pub(super) const COMMAND_BAR_RADIUS: f32 = 14.0;
pub(super) const COMMAND_BAR_FIELD_HEIGHT: f32 = 52.0;
pub(super) const COMMAND_BAR_FIELD_PADDING: f32 = 16.0;
pub(super) const COMMAND_BAR_FONT_SIZE: f32 = 18.0;
pub(super) const COMMAND_BAR_ICON_SIZE: f32 = 18.0;
pub(super) const COMMAND_BAR_ROW_HEIGHT: f32 = 40.0;
pub(super) const COMMAND_BAR_ROWS_INSET: f32 = 6.0;
pub(super) const COMMAND_BAR_DETAIL_FONT_SIZE: f32 = 12.0;
