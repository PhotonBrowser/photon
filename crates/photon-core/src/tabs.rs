//! Rules for a window's tabs: pinned tabs, which come first and are saved so
//! they open again, and closing tabs left unused.

use std::time::Duration;

/// Unpinned tabs left unused this long are closed when archiving is on.
pub const ARCHIVE_AFTER: Duration = Duration::from_secs(12 * 60 * 60);

/// Whether a tab is closed for being left unused: archiving is on, and it is
/// neither pinned nor the tab in use, and has not been used for
/// [`ARCHIVE_AFTER`].
pub fn should_archive(enabled: bool, pinned: bool, active: bool, idle: Duration) -> bool {
    enabled && !pinned && !active && idle >= ARCHIVE_AFTER
}

/// Whether a tab dropped onto the tab at `target` becomes pinned, when the
/// window's first `pinned_count` tabs are pinned: it joins the section it
/// is dropped in.
pub fn pinned_after_drop(target: usize, pinned_count: usize) -> bool {
    target < pinned_count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archives_only_unused_unpinned_tabs() {
        let long = ARCHIVE_AFTER;
        assert!(should_archive(true, false, false, long));
        assert!(!should_archive(false, false, false, long));
        assert!(!should_archive(true, true, false, long));
        assert!(!should_archive(true, false, true, long));
        assert!(!should_archive(true, false, false, long / 2));
    }

    #[test]
    fn a_dropped_tab_joins_the_section_it_lands_in() {
        assert!(pinned_after_drop(1, 2));
        assert!(!pinned_after_drop(2, 2));
        assert!(!pinned_after_drop(0, 0));
    }
}
