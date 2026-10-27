//! Shared UI measurements in logical pixels.
//!
//! Keep repeated geometry, spacing, and type sizes here so shell views stay
//! visually consistent and can be tuned without searching individual views.

pub(super) const TITLEBAR_HEIGHT: f32 = 36.0;
pub(super) const TAB_HEIGHT: f32 = 28.0;
/// A pinned tab in the strip, which shows only its icon.
pub(super) const PINNED_TAB_WIDTH: f32 = 36.0;
pub(super) const TAB_STRIP_GAP: f32 = 2.0;
pub(super) const TAB_STRIP_INSET: f32 = 4.0;
pub(super) const TAB_CLOSE_GAP: f32 = 6.0;
/// Pointer travel before a press becomes a tab drag.
pub(super) const TAB_DRAG_THRESHOLD: f32 = 4.0;
/// Edge zone and maximum speed for scrolling a tab strip during a drag.
pub(super) const TAB_DRAG_SCROLL_EDGE: f32 = 28.0;
pub(super) const TAB_DRAG_SCROLL_SPEED: f32 = 720.0;
pub(super) const TAB_REORDER_DURATION_MS: u64 = 150;
pub(super) const TAB_INSERT_PUSH_DURATION_MS: u64 = 120;
pub(super) const TAB_DROP_SETTLE_DURATION_MS: u64 = 120;
pub(super) const TAB_ENTRY_OFFSET: f32 = 5.0;
pub(super) const TAB_MAX_WIDTH: f32 = 220.0;
pub(super) const TAB_HORIZONTAL_PADDING: f32 = 10.0;
pub(super) const TAB_CLOSE_BUTTON_SIZE: f32 = 24.0;
pub(super) const TAB_ICON_SIZE: f32 = 12.0;
pub(super) const TAB_FAVICON_SIZE: f32 = 16.0;
pub(super) const TAB_AUDIO_BUTTON_SIZE: f32 = 24.0;
pub(super) const TAB_AUDIO_ICON_SIZE: f32 = TAB_FAVICON_SIZE;
pub(super) const TAB_FONT_SIZE: f32 = 12.0;
/// The narrowest unpinned tab that fits its icon, close control, gaps, and
/// horizontal padding without clipping when the strip is crowded.
pub(super) const TAB_MIN_WIDTH: f32 =
    TAB_FAVICON_SIZE + TAB_CLOSE_BUTTON_SIZE + 2.0 * TAB_CLOSE_GAP + 2.0 * TAB_HORIZONTAL_PADDING;
/// Audio tabs replace the favicon with a larger control.
pub(super) const TAB_AUDIO_MIN_WIDTH: f32 = TAB_AUDIO_BUTTON_SIZE
    + TAB_CLOSE_BUTTON_SIZE
    + 2.0 * TAB_CLOSE_GAP
    + 2.0 * TAB_HORIZONTAL_PADDING;

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
pub(super) const SITE_INFO_WIDTH: f32 = 280.0;

/// A keycap in a keyboard shortcut.
pub(super) const KBD_HEIGHT: f32 = 18.0;
pub(super) const KBD_PADDING: f32 = 4.0;
pub(super) const KBD_FONT_SIZE: f32 = 11.0;
/// Space between a shortcut's keycaps.
pub(super) const KBD_GAP: f32 = 2.0;
/// Space above the first and below the last menu row.
pub(super) const MENU_PADDING: f32 = 4.0;
pub(super) const MENU_FONT_SIZE: f32 = 13.0;
pub(super) const MENU_ITEM_HEIGHT: f32 = 28.0;
pub(super) const MENU_ITEM_HORIZONTAL_PADDING: f32 = 12.0;
pub(super) const MENU_ITEM_GAP: f32 = 2.0;
pub(super) const MENU_SEPARATOR_HEIGHT: f32 = 1.0;
/// Space above and below a menu separator.
pub(super) const MENU_SEPARATOR_MARGIN: f32 = 4.0;
/// The shared weight of a one-pixel outline, used when deriving nested radii.
pub(super) const SURFACE_BORDER_WIDTH: f32 = 1.0;

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
pub(super) const CHIP_PADDING: f32 = 10.0;
/// Right padding beside a chip's buttons, which bring their own.
pub(super) const CHIP_TRAILING_PADDING: f32 = 3.0;
pub(super) const CHIP_GAP: f32 = 6.0;
pub(super) const CHIP_ICON_SIZE: f32 = 12.0;
pub(super) const CHIP_FONT_SIZE: f32 = 12.0;
pub(super) const CHIP_CLOSE_SIZE: f32 = 24.0;
pub(super) const CHIP_CLOSE_ICON_SIZE: f32 = 10.0;

pub(super) const BUTTON_HEIGHT: f32 = 30.0;
pub(super) const BUTTON_MIN_WIDTH: f32 = 76.0;
pub(super) const BUTTON_HORIZONTAL_PADDING: f32 = 16.0;
pub(super) const BUTTON_FONT_SIZE: f32 = 13.0;
pub(super) const BUTTON_GAP: f32 = 8.0;
pub(super) const BUTTON_JIGGLE_RANGE: f32 = 4.0;
pub(super) const BUTTON_JIGGLE_RESISTANCE: f32 = 0.10;
pub(super) const ICON_BUTTON_SIZE: f32 = 24.0;
pub(super) const ICON_BUTTON_ICON_SIZE: f32 = 13.0;
pub(super) const SMALL_BUTTON_HEIGHT: f32 = 24.0;
pub(super) const SMALL_BUTTON_HORIZONTAL_PADDING: f32 = 9.0;
pub(super) const SMALL_BUTTON_FONT_SIZE: f32 = 12.0;

pub(super) const TOOLTIP_FONT_SIZE: f32 = 12.0;
pub(super) const TOOLTIP_HORIZONTAL_PADDING: f32 = 10.0;
pub(super) const TOOLTIP_VERTICAL_PADDING: f32 = 6.0;

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
pub(super) const COMMAND_PALETTE_WIDTH: f32 = 520.0;
pub(super) const COMMAND_PALETTE_MAX_HEIGHT: f32 = 560.0;
pub(super) const COMMAND_PALETTE_MAX_RESULTS_HEIGHT: f32 = 448.0;
pub(super) const COMMAND_PALETTE_INPUT_HEIGHT: f32 = 48.0;
pub(super) const COMMAND_PALETTE_ROW_HEIGHT: f32 = 32.0;
pub(super) const COMMAND_PALETTE_PADDING: f32 = 14.0;
pub(super) const COMMAND_PALETTE_ROW_INSET: f32 = 10.0;
pub(super) const COMMAND_PALETTE_LIST_PADDING: f32 = 4.0;
pub(super) const COMMAND_PALETTE_FOOTER_HEIGHT: f32 = 30.0;
pub(super) const COMMAND_PALETTE_FOOTER_GAP: f32 = 12.0;

/// The address field in the toolbar.
pub(super) const OMNIBOX_HEIGHT: f32 = 28.0;
/// The larger address field in the sidebar.
pub(super) const SIDEBAR_OMNIBOX_HEIGHT: f32 = 38.0;
pub(super) const SIDEBAR_OMNIBOX_FONT_SIZE: f32 = 15.0;
pub(super) const SIDEBAR_OMNIBOX_BUTTON_INSET: f32 = 4.0;
pub(super) const SIDEBAR_OMNIBOX_TEXT_INSET: f32 = TOOLBAR_BUTTON_SIZE + SIDEBAR_PADDING;
pub(super) const OMNIBOX_TEXT_TRANSITION_OPACITY: f32 = 0.5;
pub(super) const OMNIBOX_BORDER_WIDTH: f32 = 1.0;
pub(super) const OMNIBOX_HORIZONTAL_PADDING: f32 = 12.0;
pub(super) const OMNIBOX_GAP: f32 = 8.0;
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
pub(super) const SETTINGS_APPEARANCE_MAX_WIDTH: f32 = 900.0;
pub(super) const THEME_PREVIEW_HEIGHT: f32 = 88.0;
pub(super) const THEME_PREVIEW_SIDEBAR_WIDTH: f32 = 48.0;
pub(super) const THEME_PREVIEW_INSET: f32 = 5.0;
pub(super) const THEME_PREVIEW_GAP: f32 = 5.0;
pub(super) const THEME_PREVIEW_LINE_HEIGHT: f32 = 4.0;
pub(super) const THEME_PREVIEW_TITLE_WIDTH: f32 = 48.0;
pub(super) const THEME_PREVIEW_SIDEBAR_INSET: f32 = 2.0;
pub(super) const THEME_PREVIEW_SIDEBAR_GAP: f32 = 2.0;
pub(super) const THEME_PREVIEW_ADDRESS_HEIGHT: f32 = 9.0;
pub(super) const THEME_PREVIEW_ADDRESS_LINE_WIDTH: f32 = 20.0;
pub(super) const THEME_PREVIEW_TAB_HEIGHT: f32 = 10.0;
pub(super) const THEME_PREVIEW_TAB_GAP: f32 = 2.0;
pub(super) const THEME_PREVIEW_TAB_TITLE_WIDTH: f32 = 17.0;
pub(super) const THEME_PREVIEW_SELECTION_INSET: f32 = 2.0;
pub(super) const THEME_PREVIEW_SELECTION_BORDER_WIDTH: f32 = 2.0;
pub(super) const ACCENT_SWATCH_SIZE: f32 = 24.0;
pub(super) const ACCENT_SWATCH_RING_SIZE: f32 = 32.0;
pub(super) const ACCENT_SWATCH_GAP: f32 = 4.0;
pub(super) const ACCENT_SECTION_PADDING: f32 = 12.0;
pub(super) const THEME_ACCENT_DIVIDER_HEIGHT: f32 = 1.0;
pub(super) const PERFORMANCE_PAGE_INSET: f32 = 32.0;
pub(super) const PERFORMANCE_PAGE_MAX_WIDTH: f32 = 1040.0;
pub(super) const PERFORMANCE_PAGE_SECTION_GAP: f32 = 24.0;
pub(super) const PERFORMANCE_PAGE_GAP: f32 = 16.0;
pub(super) const PERFORMANCE_CARD_PADDING: f32 = 14.0;
pub(super) const PERFORMANCE_TASK_GAP: f32 = 8.0;
pub(super) const PERFORMANCE_CHART_HEIGHT: f32 = 112.0;
pub(super) const PERFORMANCE_EVENT_LOOP_DELAY_THRESHOLD_MS: u64 = 50;
pub(super) const TASK_MANAGER_INSET: f32 = 18.0;
pub(super) const TASK_MANAGER_SECTION_GAP: f32 = 14.0;
pub(super) const TASK_MANAGER_CARD_GAP: f32 = 10.0;
pub(super) const TASK_MANAGER_CARD_PADDING: f32 = 12.0;
pub(super) const TASK_MANAGER_TABLE_HEADER_HEIGHT: f32 = 32.0;
pub(super) const TASK_MANAGER_TABLE_ROW_HEIGHT: f32 = 48.0;
pub(super) const TASK_MANAGER_COLUMN_GAP: f32 = 10.0;
pub(super) const TASK_MANAGER_PID_WIDTH: f32 = 68.0;
pub(super) const TASK_MANAGER_CPU_WIDTH: f32 = 78.0;
pub(super) const TASK_MANAGER_MEMORY_WIDTH: f32 = 112.0;
pub(super) const TASK_MANAGER_DISK_WIDTH: f32 = 154.0;
pub(super) const TASK_MANAGER_NETWORK_WIDTH: f32 = 164.0;
pub(super) const TASK_MANAGER_PROCESS_MIN_WIDTH: f32 = 220.0;
pub(super) const SWITCH_WIDTH: f32 = 45.0;
pub(super) const SWITCH_KNOB_WIDTH: f32 = 26.0;
pub(super) const SWITCH_KNOB_HEIGHT: f32 = 16.0;
pub(super) const SWITCH_INSET: f32 = 2.0;
pub(super) const SWITCH_KNOB_HELD_STRETCH: f32 = 6.0;
pub(super) const CHECKBOX_SIZE: f32 = 16.0;
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
pub(super) const SEGMENT_GAP: f32 = 2.0;
pub(super) const SEGMENT_INSET: f32 = 2.0;
pub(super) const INTENSITY_SLIDER_HEIGHT: f32 = 26.0;
pub(super) const INTENSITY_SLIDER_THUMB_WIDTH: f32 = 18.0;
pub(super) const INTENSITY_SLIDER_THUMB_HEIGHT: f32 = 14.0;
pub(super) const INTENSITY_SLIDER_TRACK_HEIGHT: f32 = 4.0;
pub(super) const INTENSITY_SLIDER_RUBBERBAND_RANGE: f32 = 36.0;
pub(super) const INTENSITY_SLIDER_RUBBERBAND_RESISTANCE: f32 = 0.55;
pub(super) const INTENSITY_SLIDER_VALUE_WIDTH: f32 = 40.0;
pub(super) const SETTINGS_ROW_HEIGHT: f32 = 34.0;
pub(super) const SETTINGS_SIDEBAR_WIDTH: f32 = 200.0;
pub(super) const SETTINGS_SIDEBAR_ITEM_HEIGHT: f32 = 30.0;
pub(super) const SETTINGS_SIDEBAR_ICON_SIZE: f32 = 14.0;

// The sidebar: navigation, address field, favourites, tabs and its footer.
/// Width of the hit area centred on the sidebar's resize edge.
pub(super) const SIDEBAR_RESIZE_HANDLE_WIDTH: f32 = 36.0;
/// Distance used to dampen a temporary stretch beyond the sidebar's maximum.
pub(super) const SIDEBAR_RESIZE_RUBBERBAND_RANGE: f32 = 56.0;
/// Resistance applied as the sidebar is pulled beyond its maximum width.
pub(super) const SIDEBAR_RESIZE_RUBBERBAND_RESISTANCE: f32 = 0.55;
/// The sidebar collapsed to a column of icons.
pub(super) const SIDEBAR_COLLAPSED_WIDTH: f32 = photon_core::SIDEBAR_COLLAPSED_WIDTH as f32;
/// Space between the collapsed sidebar's edges and its icons.
pub(super) const SIDEBAR_COLLAPSED_PADDING: f32 = 6.0;
/// A tab, favourite or button in the collapsed sidebar: a square that fills
/// it between its padding.
pub(super) const SIDEBAR_COLLAPSED_ITEM_SIZE: f32 =
    SIDEBAR_COLLAPSED_WIDTH - SIDEBAR_COLLAPSED_PADDING * 2.0;
pub(super) const SIDEBAR_COLLAPSED_ICON_SIZE: f32 = 16.0;
/// Space between the collapsed sidebar's icons, and around its dividers.
pub(super) const SIDEBAR_COLLAPSED_GAP: f32 = 4.0;
/// Space between the sidebar's edges and its content.
pub(super) const SIDEBAR_PADDING: f32 = 8.0;
/// Space between the sidebar's top-row buttons.
pub(super) const SIDEBAR_CONTROL_GAP: f32 = 8.0;
/// Space between the sidebar's sections.
pub(super) const SIDEBAR_SECTION_GAP: f32 = 8.0;
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

pub(super) fn control_radius() -> f32 {
    crate::platform::theme_config::radius_tokens().control
}

pub(super) fn small_button_radius() -> f32 {
    crate::platform::theme_config::radius_tokens().small_button
}

pub(super) fn keycap_radius() -> f32 {
    crate::platform::theme_config::radius_tokens().keycap
}

pub(super) fn surface_radius() -> f32 {
    crate::platform::theme_config::radius_tokens().surface
}

/// The corner radius of an outer surface follows the inner surface through
/// its inset and border thickness.
pub(super) const fn concentric_radius(inner: f32, inset: f32, border: f32) -> f32 {
    inner + inset + border
}

pub(super) fn menu_surface_radius() -> f32 {
    concentric_radius(menu_item_radius(), MENU_PADDING, SURFACE_BORDER_WIDTH)
}

pub(super) fn menu_item_radius() -> f32 {
    crate::platform::theme_config::radius_tokens().menu_item
}

pub(super) fn chip_radius() -> f32 {
    concentric_radius(
        small_button_radius(),
        (CHIP_HEIGHT - SMALL_BUTTON_HEIGHT) / 2.0,
        0.0,
    )
}

pub(super) fn tooltip_radius() -> f32 {
    crate::platform::theme_config::radius_tokens().tooltip
}

pub(super) fn omnibox_radius() -> f32 {
    crate::platform::theme_config::radius_tokens().omnibox
}

pub(super) fn sidebar_omnibox_radius() -> f32 {
    concentric_radius(
        sidebar_omnibox_button_radius(),
        SIDEBAR_OMNIBOX_BUTTON_INSET,
        OMNIBOX_BORDER_WIDTH,
    )
}

pub(super) fn sidebar_omnibox_button_radius() -> f32 {
    crate::platform::theme_config::radius_tokens().sidebar_omnibox_button
}

pub(in crate::platform) fn page_radius() -> f32 {
    crate::platform::theme_config::radius_tokens().page
}

pub(in crate::platform) fn page_smoothing() -> f32 {
    crate::platform::theme_config::radius_tokens().page_smoothing
}

pub(super) fn theme_preview_radius() -> f32 {
    concentric_radius(control_radius(), THEME_PREVIEW_INSET, 0.0)
}

pub(super) fn theme_preview_selection_radius() -> f32 {
    concentric_radius(
        theme_preview_radius(),
        THEME_PREVIEW_SELECTION_INSET,
        THEME_PREVIEW_SELECTION_BORDER_WIDTH,
    )
}

pub(super) fn theme_accent_panel_radius() -> f32 {
    crate::platform::theme_config::radius_tokens().theme_accent_panel
}

pub(super) fn checkbox_radius() -> f32 {
    crate::platform::theme_config::radius_tokens().checkbox
}

pub(super) fn segment_radius() -> f32 {
    crate::platform::theme_config::radius_tokens().segment
}

pub(super) fn choices_radius() -> f32 {
    concentric_radius(segment_radius(), SEGMENT_INSET, SURFACE_BORDER_WIDTH)
}

pub(super) fn choices_group_radius() -> f32 {
    concentric_radius(choices_radius(), MENU_PADDING, SURFACE_BORDER_WIDTH)
}

pub(super) fn sidebar_collapsed_item_radius() -> f32 {
    crate::platform::theme_config::radius_tokens().sidebar_collapsed_item
}

pub(super) fn sidebar_item_radius() -> f32 {
    crate::platform::theme_config::radius_tokens().sidebar_item
}

pub(super) fn sidebar_reveal_radius() -> f32 {
    crate::platform::theme_config::radius_tokens().sidebar_reveal
}

pub(super) fn omnibox_panel_bottom_radius() -> f32 {
    concentric_radius(
        menu_item_radius(),
        OMNIBOX_SUGGESTIONS_INSET,
        OMNIBOX_BORDER_WIDTH,
    )
}

pub(super) fn peek_card_radius() -> f32 {
    concentric_radius(page_radius(), 0.0, SURFACE_BORDER_WIDTH)
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
/// The spaces in the sidebar's footer: a dot each, larger while shown.
pub(super) const SPACE_DOT_SIZE: f32 = 7.0;
pub(super) const SPACE_DOT_ACTIVE_SIZE: f32 = 10.0;
pub(super) const SPACE_DOT_BUTTON_SIZE: f32 = 22.0;
pub(super) const SPACE_DOT_GAP: f32 = 2.0;
/// Peek: the space around the preview, as a fraction of the window, and
/// between its buttons and the page.
pub(super) const PEEK_INSET_FRACTION: f32 = 0.08;
pub(super) const PEEK_CONTROL_GAP: f32 = 6.0;
