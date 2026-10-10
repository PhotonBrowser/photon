//! Address classification and resolution for typed omnibox input.

use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::OnceLock;

use url::Url;

use super::engines::{SearchEngine, SearchEngines};

/// Schemes that name a page on the web.
const WEB_SCHEMES: [&str; 4] = ["http", "https", "ws", "wss"];

/// Non-web schemes that should be passed through as written.
const OTHER_SCHEMES: [&str; 10] = [
    "about",
    "blob",
    "data",
    "file",
    "ftp",
    "gemini",
    "javascript",
    "mailto",
    "sms",
    "tel",
];

/// How an address reached the omnibox, which decides the scheme it is given.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UrlKind {
    /// The text already named a web scheme (`http`, `https`, `ws` or `wss`).
    Explicit,
    /// A bare host that got the secure default.
    Inferred,
    /// A loopback or private-network host, served over plain HTTP.
    Local,
    /// A scheme that is not a web address, opened exactly as written.
    Other,
}

/// What the omnibox decided the typed text means.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OmniboxTarget {
    /// Open this address.
    Url { url: String, kind: UrlKind },
    /// Search for this query.
    Search {
        url: String,
        query: String,
        engine: SearchEngine,
    },
}

impl OmniboxTarget {
    /// The address to load, whichever of the two kinds produced it.
    pub fn url(&self) -> &str {
        match self {
            Self::Url { url, .. } | Self::Search { url, .. } => url,
        }
    }

    /// Whether the text was read as a query rather than an address.
    pub fn is_search(&self) -> bool {
        matches!(self, Self::Search { .. })
    }
}

/// Why typed text cannot be opened.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OmniboxError {
    /// Nothing was typed.
    Empty,
    /// The text is address-shaped but cannot be opened, for example `https://`.
    InvalidAddress,
}

pub(crate) static BUILTIN_ENGINES: OnceLock<SearchEngines> = OnceLock::new();

/// Resolves typed text with Photon's built-in engines.
pub fn resolve(input: &str) -> Result<OmniboxTarget, OmniboxError> {
    resolve_with(input, BUILTIN_ENGINES.get_or_init(SearchEngines::builtin))
}

/// Resolves typed text with a specific engine registry.
pub fn resolve_with(input: &str, engines: &SearchEngines) -> Result<OmniboxTarget, OmniboxError> {
    let text = input.trim();
    if text.is_empty() {
        return Err(OmniboxError::Empty);
    }
    match classify(text) {
        Intent::Scheme(scheme) if is_web_scheme(&scheme) => {
            open_address(text, None, UrlKind::Explicit)
        }
        Intent::Scheme(_) => open_address(text, None, UrlKind::Other),
        Intent::LocalHost => open_address(text, Some("http"), UrlKind::Local),
        Intent::BareHost => open_address(text, Some("https"), UrlKind::Inferred),
        Intent::Query => Ok(search(text, engines)),
    }
}

/// Whether the text would be opened as an address rather than searched for.
pub fn looks_like_address(input: &str) -> bool {
    let text = input.trim();
    !text.is_empty() && !matches!(classify(text), Intent::Query)
}

enum Intent {
    /// The text named its own scheme.
    Scheme(String),
    /// A host on this machine or the local network.
    LocalHost,
    /// A bare host such as `example.com`.
    BareHost,
    /// Something to search for.
    Query,
}

fn classify(text: &str) -> Intent {
    if let Some(scheme) = explicit_scheme(text) {
        return Intent::Scheme(scheme);
    }
    if is_local_host(text) {
        return Intent::LocalHost;
    }
    if is_bare_host(text) {
        return Intent::BareHost;
    }
    Intent::Query
}

/// The scheme the text names, when it names one Photon knows.
///
/// An unknown `word:` prefix is deliberately not a scheme: `note: buy milk` is
/// a search, and reading it as an address would fail to load anything.
fn explicit_scheme(text: &str) -> Option<String> {
    let (scheme, _rest) = text.split_once(':')?;
    // A scheme with a dot in it is a host with a port, not a scheme.
    if scheme.is_empty() || scheme.contains('.') {
        return None;
    }
    if !scheme
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        || !scheme
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '+' | '-'))
    {
        return None;
    }
    let scheme = scheme.to_ascii_lowercase();
    // The browser's own pages, such as photon://settings, open as written too.
    let known = is_web_scheme(&scheme)
        || OTHER_SCHEMES.contains(&scheme.as_str())
        || scheme == photon_brand::PAGE_SCHEME;
    known.then_some(scheme)
}

fn is_web_scheme(scheme: &str) -> bool {
    WEB_SCHEMES.contains(&scheme)
}

fn open_address(
    text: &str,
    prefix: Option<&str>,
    kind: UrlKind,
) -> Result<OmniboxTarget, OmniboxError> {
    let candidate = match prefix {
        Some(scheme) => format!("{scheme}://{text}"),
        None => text.to_owned(),
    };
    let parsed = Url::parse(&candidate).map_err(|_| OmniboxError::InvalidAddress)?;
    if is_web_scheme(parsed.scheme()) && parsed.host_str().is_none() {
        return Err(OmniboxError::InvalidAddress);
    }
    Ok(OmniboxTarget::Url {
        url: parsed.to_string(),
        kind,
    })
}

pub(crate) fn search(text: &str, engines: &SearchEngines) -> OmniboxTarget {
    let query = collapse_whitespace(text);
    let engine = engines.default_engine();
    OmniboxTarget::Search {
        url: engine.search_url(&query),
        query,
        engine,
    }
}

/// Whether the host part names this machine or the local network.
fn is_local_host(text: &str) -> bool {
    let host = host_part(text);
    let host = host.trim_matches(['[', ']']);
    if host.eq_ignore_ascii_case("localhost") || host.to_ascii_lowercase().ends_with(".localhost") {
        return true;
    }
    if let Ok(address) = host.parse::<Ipv4Addr>() {
        return address.is_loopback() || is_private_v4(address);
    }
    match host.parse::<Ipv6Addr>() {
        Ok(address) => address.is_loopback() || is_local_v6(&address),
        Err(_) => false,
    }
}

fn is_private_v4(address: Ipv4Addr) -> bool {
    let [first, second, ..] = address.octets();
    first == 10
        || (first == 172 && (16..=31).contains(&second))
        || (first == 192 && second == 168)
        || (first == 169 && second == 254)
}

/// Unique-local (`fc00::/7`) and link-local (`fe80::/10`) addresses.
fn is_local_v6(address: &Ipv6Addr) -> bool {
    match address.segments() {
        [0xfc00..=0xfdff, ..] | [0xfe80, ..] => true,
        _ => false,
    }
}

/// Whether the host part is a plausible public or local hostname.
fn is_bare_host(text: &str) -> bool {
    // Whitespace means this is a sentence, however much of it looks like a host.
    if text.chars().any(char::is_whitespace) {
        return false;
    }
    let host = host_part(text);
    // A single label is a name, not an address, unless it is a loopback host,
    // which `is_local_host` already claimed.
    if !host.contains('.') || host.len() > 253 {
        return false;
    }
    let mut labels = host.split('.');
    let last = match labels.next_back() {
        Some(label) => label,
        None => return false,
    };
    for label in host.split('.') {
        if !is_host_label(label) {
            return false;
        }
    }
    if last.chars().all(|character| character.is_ascii_digit()) {
        // A numeric last label is an IP address, and only a real one counts:
        // `1.2.3.4.5` and `3.14` are searches.
        return last.parse::<Ipv4Addr>().is_ok();
    }
    // An internationalized top level domain is punycode, not a plain word.
    if last.starts_with("xn--") {
        return last.len() > 4;
    }
    last.len() >= 2
        && last
            .chars()
            .all(|character| character.is_ascii_alphabetic())
}

fn is_host_label(label: &str) -> bool {
    !label.is_empty()
        && label.len() <= 63
        && !label.starts_with('-')
        && !label.ends_with('-')
        && label
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
}

/// The host part of typed text: everything before a path, query or fragment,
/// and without a port.
fn host_part(text: &str) -> &str {
    let authority = text.split(['/', '?', '#']).next().unwrap_or(text).trim();
    if let Some(rest) = authority.strip_prefix('[') {
        // Bracketed IPv6 literal: the port, if any, follows the bracket.
        return rest.split(']').next().unwrap_or(rest);
    }
    match authority.rsplit_once(':') {
        Some((host, port)) if port.parse::<u16>().is_ok() => host,
        _ => authority,
    }
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines::{DUCKDUCKGO, GOOGLE, SearchEngineError};
    use std::borrow::Cow;

    fn url_of(input: &str) -> String {
        resolve(input).expect("resolves").url().to_owned()
    }

    fn kind_of(input: &str) -> UrlKind {
        match resolve(input).expect("resolves") {
            OmniboxTarget::Url { kind, .. } => kind,
            OmniboxTarget::Search { .. } => panic!("{input} was read as a query"),
        }
    }

    #[test]
    fn opens_the_browsers_own_pages_as_written() {
        let address = format!("{}://settings", photon_brand::PAGE_SCHEME);
        assert_eq!(url_of(&address), address);
        assert_eq!(kind_of(&address), UrlKind::Other);
    }

    #[test]
    fn opens_text_that_already_names_a_web_scheme() {
        assert_eq!(url_of("http://example.com/foo"), "http://example.com/foo");
        assert_eq!(kind_of("http://example.com"), UrlKind::Explicit);
        assert_eq!(url_of("https://example.com"), "https://example.com/");
        assert_eq!(url_of("HTTPS://Example.COM"), "https://example.com/");
    }

    #[test]
    fn gives_a_bare_host_the_secure_default() {
        assert_eq!(url_of("example.com"), "https://example.com/");
        assert_eq!(kind_of("example.com"), UrlKind::Inferred);
        assert_eq!(
            url_of("example.com/path?q=1#top"),
            "https://example.com/path?q=1#top"
        );
        assert_eq!(
            url_of("example.com:8443/admin"),
            "https://example.com:8443/admin"
        );
        assert_eq!(url_of("sub.example.co.uk"), "https://sub.example.co.uk/");
        assert_eq!(kind_of("hello.world"), UrlKind::Inferred);
    }

    #[test]
    fn serves_local_addresses_over_plain_http() {
        assert_eq!(url_of("localhost"), "http://localhost/");
        assert_eq!(url_of("localhost:3000"), "http://localhost:3000/");
        assert_eq!(url_of("LOCALHOST:3000/x"), "http://localhost:3000/x");
        assert_eq!(url_of("127.0.0.1:8080"), "http://127.0.0.1:8080/");
        assert_eq!(url_of("[::1]:9000"), "http://[::1]:9000/");
        assert_eq!(url_of("192.168.1.10"), "http://192.168.1.10/");
        assert_eq!(url_of("10.0.0.5:8080"), "http://10.0.0.5:8080/");
        assert_eq!(kind_of("localhost:3000"), UrlKind::Local);
    }

    #[test]
    fn keeps_schemes_that_are_not_web_addresses() {
        assert_eq!(
            url_of("mailto:person@example.com"),
            "mailto:person@example.com"
        );
        assert_eq!(url_of("about:blank"), "about:blank");
        assert_eq!(url_of("data:text/plain,hi"), "data:text/plain,hi");
        assert_eq!(kind_of("about:blank"), UrlKind::Other);
    }

    #[test]
    fn searches_for_anything_that_is_not_an_address() {
        for query in [
            "how to bake bread",
            "what is 2 + 2",
            "note: buy milk",
            "python",
            "1.2.3.4.5",
            "3.14",
            "999.999.999.999",
            "who ordered the extra napkins",
            "example .com",
        ] {
            let resolved = resolve(query).expect("resolves");
            assert!(resolved.is_search(), "{query} should be a search");
            assert!(
                resolved
                    .url()
                    .starts_with("https://www.google.com/search?q="),
                "{query} resolved to {}",
                resolved.url()
            );
        }
    }

    #[test]
    fn searches_with_the_query_encoded_and_collapsed() {
        let OmniboxTarget::Search { url, query, .. } =
            resolve("  how   to bake bread &  cakes ").expect("resolves")
        else {
            panic!("expected a search");
        };
        assert_eq!(query, "how to bake bread & cakes");
        assert_eq!(
            url,
            "https://www.google.com/search?q=how%20to%20bake%20bread%20%26%20cakes"
        );
    }

    #[test]
    fn encodes_a_non_ascii_query() {
        let OmniboxTarget::Search { url, .. } = resolve("crème brûlée").expect("resolves")
        else {
            panic!("expected a search");
        };
        assert_eq!(
            url,
            "https://www.google.com/search?q=cr%C3%A8me%20br%C3%BBl%C3%A9e"
        );
    }

    #[test]
    fn reports_empty_and_unopenable_input() {
        assert_eq!(resolve(""), Err(OmniboxError::Empty));
        assert_eq!(resolve("   "), Err(OmniboxError::Empty));
        assert_eq!(resolve("https://"), Err(OmniboxError::InvalidAddress));
    }

    #[test]
    fn answers_whether_text_would_open_as_an_address() {
        assert!(looks_like_address("example.com"));
        assert!(looks_like_address("  https://example.com "));
        assert!(!looks_like_address("bake bread"));
        assert!(!looks_like_address(""));
    }

    #[test]
    fn uses_the_registry_default_engine() {
        let mut engines = SearchEngines::builtin();
        assert_eq!(engines.default_engine(), GOOGLE);
        assert!(engines.set_default("duckduckgo"));
        assert!(!engines.set_default("nonesuch"));

        let OmniboxTarget::Search { url, engine, .. } =
            resolve_with("rust lang", &engines).expect("resolves")
        else {
            panic!("expected a search");
        };
        assert_eq!(engine, DUCKDUCKGO);
        assert_eq!(url, "https://duckduckgo.com/?q=rust%20lang");
    }

    #[test]
    fn accepts_engines_registered_later() {
        let mut engines = SearchEngines::default();
        let kagi = SearchEngine {
            id: Cow::Borrowed("kagi"),
            name: Cow::Borrowed("Kagi"),
            template: Cow::Borrowed("https://kagi.com/search?q={query}"),
        };
        engines.register(kagi.clone()).expect("registers");
        assert!(engines.set_default("kagi"));
        assert_eq!(engines.default_engine(), kagi);
        assert_eq!(engines.get("kagi"), Some(kagi));

        let target = resolve_with("photon browser", &engines).expect("resolves");
        assert_eq!(target.url(), "https://kagi.com/search?q=photon%20browser");
    }

    #[test]
    fn rejects_an_engine_that_cannot_take_a_query() {
        let mut engines = SearchEngines::default();
        let missing = SearchEngine {
            id: Cow::Borrowed("broken"),
            name: Cow::Borrowed("Broken"),
            template: Cow::Borrowed("https://broken.test/search"),
        };
        assert_eq!(
            engines.register(missing),
            Err(SearchEngineError::MissingQueryPlaceholder)
        );
        let not_a_url = SearchEngine {
            id: Cow::Borrowed("relative"),
            name: Cow::Borrowed("Relative"),
            template: Cow::Borrowed("/search?q={query}"),
        };
        assert_eq!(
            engines.register(not_a_url),
            Err(SearchEngineError::InvalidEndpoint)
        );
        // A registry that lost its default still searches.
        assert_eq!(engines.default_engine(), GOOGLE);
    }

    #[test]
    fn re_registering_an_engine_replaces_it() {
        let mut engines = SearchEngines::builtin();
        engines
            .register(SearchEngine {
                id: Cow::Borrowed("google"),
                name: Cow::Borrowed("Google"),
                template: Cow::Borrowed("https://google.com/search?q={query}"),
            })
            .expect("registers");
        assert_eq!(engines.engines().len(), 5);
        assert_eq!(
            engines.default_engine().template,
            "https://google.com/search?q={query}"
        );
    }

    #[test]
    fn every_builtin_engine_builds_a_usable_url() {
        for engine in SearchEngines::builtin().engines() {
            assert!(engine.is_valid(), "{} is not usable", engine.id);
            let url = engine.search_url("photon browser");
            assert!(Url::parse(&url).is_ok(), "{} produced {url}", engine.id);
            assert!(
                url.contains("photon%20browser"),
                "{} produced {url}",
                engine.id
            );
        }
    }
}
