//! The side dock's navigation controls: in a row beside the window controls,
//! or, collapsed to icons, the toggle beneath the stacked window controls.

use gpui::{AnyElement, div, prelude::*, px};

use super::super::layout::{h_stack, v_stack};
use super::super::metrics;
use crate::platform::window_settings;

pub(in super::super) fn navigation_bar(
    width: f32,
    fullscreen: bool,
    leading: bool,
    toggle: Option<AnyElement>,
    controls: Vec<AnyElement>,
    tight: bool,
) -> gpui::Div {
    let inset = sidebar_header_inset(fullscreen, leading);
    // A narrow header reserves only the controls' measured footprint, and
    // its buttons sit edge to edge, so back, forward and the overflow menu
    // still fit at the minimum width.
    let inset = if tight {
        inset.min(metrics::SIDEBAR_HEADER_NARROW_INSET)
    } else {
        inset
    };
    let gap = if tight {
        0.0
    } else {
        metrics::SIDEBAR_CONTROL_GAP
    };
    let mut contents = h_stack()
        .w(px(width))
        .h(px(metrics::TITLEBAR_HEIGHT))
        .flex_shrink_0()
        .items_center()
        .gap(px(gap))
        .pr(px(metrics::SIDEBAR_PADDING))
        .tab_group();
    contents = if leading {
        contents.pl(px(inset))
    } else {
        contents.pl(px(metrics::SIDEBAR_PADDING))
    };
    contents
        .child(toggle.unwrap_or_else(|| div().into_any_element()))
        .child(div().flex_1())
        .child(h_stack().gap(px(gap)).children(controls))
}

/// The sidebar header's reserve for the native window controls.
pub(in super::super) fn sidebar_header_inset(fullscreen: bool, leading: bool) -> f32 {
    if fullscreen {
        0.0
    } else if leading {
        window_settings::native_controls_inset()
    } else {
        metrics::SIDEBAR_PADDING
    }
}

/// How the sidebar header fits its buttons at `width`: everything inline, or
/// the toggle folded into the overflow menu, or the toggle, reload and menu
/// folded, keeping only back and forward inline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) enum HeaderTier {
    Wide,
    Compact,
    Narrow,
}

pub(in super::super) fn header_tier(
    width: f32,
    inset: f32,
    has_toggle: bool,
    control_count: usize,
) -> HeaderTier {
    let button = metrics::TOOLBAR_BUTTON_SIZE;
    let gap = metrics::SIDEBAR_CONTROL_GAP;
    let pad = metrics::SIDEBAR_PADDING;
    let n = control_count as f32;
    let inner = control_count.saturating_sub(1) as f32 * gap;
    let wide =
        inset + (if has_toggle { 2.0 * gap + button } else { gap }) + n * button + inner + pad;
    if width >= wide {
        return HeaderTier::Wide;
    }
    // Tight rows use smaller plates with no gaps, and join the overflow
    // button to the group, so folding the toggle still needs its own tier.
    let narrow_button = metrics::SIDEBAR_HEADER_NARROW_BUTTON;
    let narrow_inset = inset.min(metrics::SIDEBAR_HEADER_NARROW_INSET);
    let compact = narrow_inset + (n + 1.0) * narrow_button + pad;
    if width >= compact {
        HeaderTier::Compact
    } else {
        HeaderTier::Narrow
    }
}

/// The collapsed sidebar's top: room for the window controls stacked at the
/// leading edge, then the toggle that expands it.
pub(in super::super) fn navigation_column(
    fullscreen: bool,
    leading: bool,
    toggle: Option<AnyElement>,
) -> gpui::Div {
    let controls_inset = if leading && !fullscreen {
        window_settings::native_controls_column_inset()
    } else {
        metrics::SIDEBAR_PADDING
    };
    v_stack()
        .w(px(metrics::SIDEBAR_COLLAPSED_WIDTH))
        .flex_shrink_0()
        .items_center()
        .pt(px(controls_inset))
        .pb(px(metrics::SIDEBAR_COLLAPSED_GAP))
        .tab_group()
        .children(toggle)
}

#[cfg(test)]
mod tests {
    use super::{HeaderTier, header_tier};

    #[test]
    fn wide_header_fits_everything() {
        assert_eq!(header_tier(240.0, 80.0, true, 3), HeaderTier::Wide);
        assert_eq!(header_tier(160.0, 8.0, true, 3), HeaderTier::Wide);
    }

    #[test]
    fn narrower_header_folds_the_toggle_first() {
        assert_eq!(header_tier(200.0, 80.0, true, 3), HeaderTier::Compact);
        assert_eq!(header_tier(184.0, 80.0, true, 3), HeaderTier::Compact);
    }

    #[test]
    fn narrowest_header_folds_reload_too() {
        assert_eq!(header_tier(183.0, 80.0, true, 3), HeaderTier::Narrow);
        assert_eq!(header_tier(160.0, 80.0, true, 3), HeaderTier::Narrow);
    }

    #[test]
    fn thresholds_cover_a_menu_button_in_the_header() {
        // Four controls need room for the overflow button as well.
        assert_eq!(header_tier(210.0, 80.0, true, 4), HeaderTier::Compact);
        assert_eq!(header_tier(209.0, 80.0, true, 4), HeaderTier::Narrow);
    }

    #[test]
    fn empty_header_never_folds() {
        assert_eq!(header_tier(160.0, 80.0, true, 0), HeaderTier::Wide);
        assert_eq!(header_tier(160.0, 80.0, false, 0), HeaderTier::Wide);
    }
}
