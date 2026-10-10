//! Requests to delete what the browser keeps about browsing.

/// What to delete, and from when. Photon's own data (history and searches)
/// is cleared by [`History::clear`](super::History::clear); website data
/// (cache and site data) by the Engine.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ClearBrowsingData {
    /// Delete what was last used at or after this time, in Unix seconds.
    /// Zero deletes everything.
    pub since: u64,
    /// Pages visited, with their icons.
    pub history: bool,
    /// Queries searched for from the omnibox.
    pub searches: bool,
    /// The Engine's network cache.
    pub cache: bool,
    /// Cookies and site storage such as localStorage and IndexedDB.
    pub site_data: bool,
}

impl ClearBrowsingData {
    /// Everything, from all time.
    pub const fn everything() -> Self {
        Self {
            since: 0,
            history: true,
            searches: true,
            cache: true,
            site_data: true,
        }
    }

    /// Whether the request deletes anything the Engine keeps.
    pub const fn touches_engine(&self) -> bool {
        self.cache || self.site_data
    }
}
