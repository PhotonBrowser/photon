//! Search engine definitions and the configurable engine registry.

use std::borrow::Cow;

use url::Url;

/// Placeholder replaced with the percent-encoded query.
pub(super) const QUERY_PLACEHOLDER: &str = "{query}";

/// A search engine Photon can hand a query to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchEngine {
    /// Stable identifier, for preferences and for [`SearchEngines::set_default`].
    pub id: Cow<'static, str>,
    /// Human-readable name.
    pub name: Cow<'static, str>,
    /// Endpoint with one [`QUERY_PLACEHOLDER`] for the encoded query.
    pub template: Cow<'static, str>,
}

pub const GOOGLE: SearchEngine = SearchEngine {
    id: Cow::Borrowed("google"),
    name: Cow::Borrowed("Google"),
    template: Cow::Borrowed("https://www.google.com/search?q={query}"),
};

pub const DUCKDUCKGO: SearchEngine = SearchEngine {
    id: Cow::Borrowed("duckduckgo"),
    name: Cow::Borrowed("DuckDuckGo"),
    template: Cow::Borrowed("https://duckduckgo.com/?q={query}"),
};

pub const BING: SearchEngine = SearchEngine {
    id: Cow::Borrowed("bing"),
    name: Cow::Borrowed("Bing"),
    template: Cow::Borrowed("https://www.bing.com/search?q={query}"),
};

pub const ECOSIA: SearchEngine = SearchEngine {
    id: Cow::Borrowed("ecosia"),
    name: Cow::Borrowed("Ecosia"),
    template: Cow::Borrowed("https://www.ecosia.org/search?q={query}"),
};

pub const WIKIPEDIA: SearchEngine = SearchEngine {
    id: Cow::Borrowed("wikipedia"),
    name: Cow::Borrowed("Wikipedia"),
    template: Cow::Borrowed("https://en.wikipedia.org/w/index.php?search={query}"),
};

impl SearchEngine {
    /// The search URL for `query`, with the query percent-encoded.
    pub fn search_url(&self, query: &str) -> String {
        let raw = self
            .template
            .replace(QUERY_PLACEHOLDER, &encode_query(query));
        match Url::parse(&raw) {
            Ok(url) => url.to_string(),
            Err(_) => raw,
        }
    }

    /// Whether this engine can actually take a query.
    ///
    /// Checked on registration so a malformed template cannot produce an
    /// unopenable address at the moment someone presses Enter.
    pub fn is_valid(&self) -> bool {
        !self.id.trim().is_empty()
            && self.template.contains(QUERY_PLACEHOLDER)
            && Url::parse(&self.template.replace(QUERY_PLACEHOLDER, "photon")).is_ok()
    }
}

/// Why an engine could not be registered.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchEngineError {
    /// The template has no `{query}` placeholder, so it cannot take a query.
    MissingQueryPlaceholder,
    /// The template is not a URL once the placeholder is filled in.
    InvalidEndpoint,
}

/// The engines an omnibox can use, and which one it uses by default.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SearchEngines {
    engines: Vec<SearchEngine>,
    /// Index into `engines`. Zero means "the first engine that was registered".
    default_index: usize,
}

impl SearchEngines {
    /// Photon's shipped engines, with Google as the default.
    pub fn builtin() -> Self {
        Self {
            engines: vec![GOOGLE, DUCKDUCKGO, BING, ECOSIA, WIKIPEDIA],
            default_index: 0,
        }
    }

    /// Every registered engine, in registration order.
    pub fn engines(&self) -> &[SearchEngine] {
        &self.engines
    }

    /// The engine queries are sent to.
    pub fn default_engine(&self) -> SearchEngine {
        self.engines
            .get(self.default_index)
            .or_else(|| self.engines.first())
            .cloned()
            .unwrap_or(GOOGLE)
    }

    /// Looks an engine up by identifier.
    pub fn get(&self, id: &str) -> Option<SearchEngine> {
        self.engines.iter().find(|engine| engine.id == id).cloned()
    }

    /// Adds an engine. An engine whose identifier is already registered
    /// replaces it, so re-registering a preference update is not an error.
    pub fn register(&mut self, engine: SearchEngine) -> Result<(), SearchEngineError> {
        if !engine.template.contains(QUERY_PLACEHOLDER) {
            return Err(SearchEngineError::MissingQueryPlaceholder);
        }
        if !engine.is_valid() {
            return Err(SearchEngineError::InvalidEndpoint);
        }
        match self
            .engines
            .iter()
            .position(|registered| registered.id == engine.id)
        {
            Some(index) => self.engines[index] = engine,
            None => self.engines.push(engine),
        }
        Ok(())
    }

    /// Chooses the engine queries go to. Returns false when `id` is unknown,
    /// leaving the current default in place.
    pub fn set_default(&mut self, id: &str) -> bool {
        match self.engines.iter().position(|engine| engine.id == id) {
            Some(index) => {
                self.default_index = index;
                true
            }
            None => false,
        }
    }
}

/// Percent-encodes a query for a search endpoint.
fn encode_query(query: &str) -> String {
    const UNRESERVED: &[u8] = b"-._~";
    let mut encoded = String::with_capacity(query.len());
    for byte in query.bytes() {
        if byte.is_ascii_alphanumeric() || UNRESERVED.contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}
