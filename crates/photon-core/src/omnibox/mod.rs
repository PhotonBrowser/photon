//! Address and search resolution, suggestion matching, and search engines.

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
    IndexedVisitedPage, PastSearch, Suggestion, SuggestionKind, Suggestions, VisitedPage,
    display_address, display_domain, match_words, normalized_display_address,
    normalized_match_text, suggest, suggest_with, suggest_with_indexed,
};
