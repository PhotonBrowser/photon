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
    ToggleDebugInfo,
}

impl BrowserCommand {
    /// Builds a navigation command from the same address-or-search rules used by the omnibox.
    pub fn from_omnibox_input(input: &str) -> Result<Self, OmniboxError> {
        resolve_omnibox_input(input).map(|target| Self::Navigate(target.url().to_owned()))
    }
}

/// Performance values exposed by the engine adapter without UI or engine types.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BrowserDiagnostics {
    pub frames_per_second: Option<f64>,
    pub cpu_percent: Option<f64>,
    pub memory_bytes: Option<u64>,
    pub download_bytes_per_second: u64,
    pub upload_bytes_per_second: u64,
    pub input_to_frame_latency_ms: Option<f64>,
    pub last_frame_interval_ms: Option<f64>,
    pub longest_frame_gap_ms: Option<f64>,
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
