//! Framework-independent address and search resolution for browser omniboxes.
//!
//! Search engine configuration lives in [`engines`]. [`resolve`] classifies
//! typed input and produces either an address or a search URL, and
//! [`suggest`] lists what the omnibox offers while typing.

mod engines;
mod resolve;
mod suggest;

pub use engines::{
    BING, DUCKDUCKGO, ECOSIA, GOOGLE, SearchEngine, SearchEngineError, SearchEngines, WIKIPEDIA,
};
pub use resolve::{
    OmniboxError, OmniboxTarget, UrlKind, looks_like_address, resolve, resolve_with,
};
pub use suggest::{
    PastSearch, Suggestion, SuggestionKind, Suggestions, VisitedPage, display_address, suggest,
    suggest_with,
};
