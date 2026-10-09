//! Framework-independent browser state and commands.

use photon_omnibox::{OmniboxError, resolve as resolve_omnibox_input};

/// The single active view's persistent browser state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BrowserState {
    pub url: String,
    pub title: String,
    pub loading: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub error: Option<String>,
}

/// Commands submitted by browser chrome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BrowserCommand {
    Navigate(String),
    Reload,
    StopLoading,
    Back,
    Forward,
    NewTab,
    NewWindow,
}

impl BrowserCommand {
    /// Builds a navigation command from the same address-or-search rules used by the omnibox.
    pub fn from_omnibox_input(input: &str) -> Result<Self, OmniboxError> {
        resolve_omnibox_input(input).map(|target| Self::Navigate(target.url().to_owned()))
    }
}

/// State delivered by the engine adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EngineEvent {
    ViewStateChanged(BrowserState),
    LoadFailed(String),
}

impl BrowserState {
    /// Applies the latest complete state snapshot from LibPhotonEmbedder.
    pub fn apply(&mut self, event: EngineEvent) {
        match event {
            EngineEvent::ViewStateChanged(state) => *self = state,
            EngineEvent::LoadFailed(message) => {
                self.loading = false;
                self.error = Some(message);
            }
        }
    }
}

/// Resolves omnibox text into the address the engine loads.
///
/// Address-shaped text opens an address. Anything else is a search query, which
/// becomes a URL on the default engine. The rules live in `photon-omnibox` so
/// the address field, the engine adapter and any future surface cannot each
/// invent their own.
pub fn normalize_url(input: &str) -> Result<String, &'static str> {
    match resolve_omnibox_input(input) {
        Ok(target) => Ok(target.url().to_owned()),
        Err(OmniboxError::Empty) => Err("Address is empty"),
        Err(OmniboxError::InvalidAddress) => Err("Invalid address"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_domains_and_paths_to_https() {
        assert_eq!(
            normalize_url("example.com").unwrap(),
            "https://example.com/"
        );
        assert_eq!(
            normalize_url("https://example.com/foo").unwrap(),
            "https://example.com/foo"
        );
    }

    #[test]
    fn searches_for_anything_that_is_not_an_address() {
        assert_eq!(
            normalize_url("how to bake bread").unwrap(),
            "https://www.google.com/search?q=how%20to%20bake%20bread"
        );
        assert_eq!(
            normalize_url("rust & golang").unwrap(),
            "https://www.google.com/search?q=rust%20%26%20golang"
        );
    }

    #[test]
    fn normalizes_local_addresses_to_http() {
        assert_eq!(
            normalize_url("localhost:3000").unwrap(),
            "http://localhost:3000/"
        );
        assert_eq!(
            normalize_url("127.0.0.1:8080").unwrap(),
            "http://127.0.0.1:8080/"
        );
        assert_eq!(
            normalize_url("http://localhost:3000").unwrap(),
            "http://localhost:3000/"
        );
    }

    #[test]
    fn preserves_explicit_non_web_schemes() {
        assert_eq!(
            normalize_url("mailto:person@example.com").unwrap(),
            "mailto:person@example.com"
        );
        assert_eq!(normalize_url("about:blank").unwrap(), "about:blank");
    }

    #[test]
    fn trims_input_and_rejects_empty_input() {
        assert_eq!(
            normalize_url("  example.com  ").unwrap(),
            "https://example.com/"
        );
        assert_eq!(normalize_url("  "), Err("Address is empty"));
    }

    #[test]
    fn applies_engine_state_snapshots() {
        let mut state = BrowserState::default();
        state.apply(EngineEvent::ViewStateChanged(BrowserState {
            url: "https://example.com/".into(),
            title: "Example".into(),
            loading: true,
            can_go_back: true,
            can_go_forward: false,
            error: None,
        }));
        assert_eq!(state.url, "https://example.com/");
        assert_eq!(state.title, "Example");
        assert!(state.loading && state.can_go_back);
        assert!(!state.can_go_forward);
    }
}
