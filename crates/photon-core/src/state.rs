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
    Back,
    Forward,
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
