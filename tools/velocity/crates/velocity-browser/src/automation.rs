use serde::{Deserialize, Serialize};

use crate::{BrowserEngine, BrowserInstallation};

/// Protocol or signal used by an adapter to control a benchmark session.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationProtocol {
    LaunchUrlReadinessBeacon,
    ChromeDevToolsProtocol,
    PhotonAutomationProtocol,
}

/// Capabilities that can be verified for a browser family by the current
/// platform adapter. These are intentionally explicit so a workload can fail
/// as unsupported before launching a browser.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BrowserCapabilities {
    pub isolated_profile: bool,
    pub resource_monitoring: bool,
    pub launch_url_navigation: bool,
    pub fixture_readiness_beacon: bool,
    pub tab_creation: bool,
    pub protocol: Option<AutomationProtocol>,
    pub tab_limitation: Option<String>,
}

/// Browser automation adapter for protocol-backed navigation and tab actions.
pub trait BrowserAutomationAdapter {
    fn protocol(&self) -> AutomationProtocol;
    fn navigate_initial_page(&mut self, url: &str) -> Result<String, String>;
    fn create_tab(&mut self, url: &str) -> Result<String, String>;
    fn page_count(&mut self) -> Result<usize, String>;
}

/// Return conservative capabilities for a discovered browser installation.
/// Resource monitoring is macOS-only; profile and automation support is based
/// on the launch adapters Velocity actually implements.
pub fn browser_capabilities(installation: &BrowserInstallation) -> BrowserCapabilities {
    let resource_monitoring = cfg!(target_os = "macos");
    match installation.metadata.engine {
        Some(BrowserEngine::Chromium) => {
            let verified_cdp = matches!(installation.metadata.id.as_str(), "chrome" | "helium");
            BrowserCapabilities {
                isolated_profile: true,
                resource_monitoring,
                launch_url_navigation: true,
                fixture_readiness_beacon: true,
                tab_creation: verified_cdp,
                protocol: Some(if verified_cdp {
                    AutomationProtocol::ChromeDevToolsProtocol
                } else {
                    AutomationProtocol::LaunchUrlReadinessBeacon
                }),
                tab_limitation: (!verified_cdp).then(|| {
                    format!(
                        "{} has no verified isolated-profile DevTools adapter; tab automation is unsupported",
                        installation.metadata.name
                    )
                }),
            }
        }
        Some(BrowserEngine::Gecko) => BrowserCapabilities {
            isolated_profile: true,
            resource_monitoring,
            launch_url_navigation: true,
            fixture_readiness_beacon: true,
            tab_creation: false,
            protocol: Some(AutomationProtocol::LaunchUrlReadinessBeacon),
            tab_limitation: Some(
                "Firefox has no verified tab-control adapter configured; Velocity will not infer tab creation from launch arguments".to_owned(),
            ),
        },
        Some(BrowserEngine::Ladybird) if installation.metadata.id.as_str() == "photon" => BrowserCapabilities {
            isolated_profile: true,
            resource_monitoring,
            launch_url_navigation: true,
            fixture_readiness_beacon: true,
            tab_creation: false,
            protocol: Some(AutomationProtocol::PhotonAutomationProtocol),
            tab_limitation: Some(
                "Photon tab and navigation capabilities are enabled only after an authenticated protocol handshake".to_owned(),
            ),
        },
        Some(BrowserEngine::Ladybird) | Some(BrowserEngine::WebKit) | None => BrowserCapabilities {
            isolated_profile: false,
            resource_monitoring,
            launch_url_navigation: true,
            fixture_readiness_beacon: true,
            tab_creation: false,
            protocol: Some(AutomationProtocol::LaunchUrlReadinessBeacon),
            tab_limitation: Some(
                "the current adapter cannot establish an isolated automation profile for this browser".to_owned(),
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{BrowserId, BrowserInstallation, BrowserMetadata, InstallationId};

    use super::*;

    fn installation(engine: BrowserEngine) -> BrowserInstallation {
        let id = match engine {
            BrowserEngine::Chromium => "chrome",
            BrowserEngine::Ladybird => "photon",
            _ => "fixture-browser",
        };
        BrowserInstallation {
            installation_id: InstallationId::new("fixture-installation").unwrap(),
            metadata: BrowserMetadata {
                id: BrowserId::new(id).unwrap(),
                name: "Fixture".to_owned(),
                bundle_identifier: None,
                engine: Some(engine),
            },
            version: Some("1".to_owned()),
            application_path: PathBuf::from("/fixture/Browser.app"),
            executable_path: PathBuf::from("/fixture/Browser"),
        }
    }

    #[test]
    fn chromium_advertises_only_implemented_cdp_tab_automation() {
        let capabilities = browser_capabilities(&installation(BrowserEngine::Chromium));
        assert!(capabilities.isolated_profile);
        assert!(capabilities.tab_creation);
        assert_eq!(
            capabilities.protocol,
            Some(AutomationProtocol::ChromeDevToolsProtocol)
        );

        let mut helium = installation(BrowserEngine::Chromium);
        helium.metadata.id = BrowserId::new("helium").unwrap();
        assert!(browser_capabilities(&helium).tab_creation);
    }

    #[test]
    fn photon_waits_for_runtime_protocol_negotiation_before_advertising_tabs() {
        let capabilities = browser_capabilities(&installation(BrowserEngine::Ladybird));
        assert!(capabilities.isolated_profile);
        assert!(capabilities.launch_url_navigation);
        assert!(capabilities.fixture_readiness_beacon);
        assert!(!capabilities.tab_creation);
        assert_eq!(
            capabilities.protocol,
            Some(AutomationProtocol::PhotonAutomationProtocol)
        );
        assert!(capabilities.tab_limitation.is_some());
    }

    #[test]
    fn unverified_chromium_forks_do_not_advertise_tab_support() {
        let mut arc = installation(BrowserEngine::Chromium);
        arc.metadata.id = BrowserId::new("arc").unwrap();
        arc.metadata.name = "Arc".to_owned();
        let capabilities = browser_capabilities(&arc);
        assert!(!capabilities.tab_creation);
        assert!(
            capabilities
                .tab_limitation
                .as_deref()
                .unwrap()
                .contains("Arc")
        );
    }

    #[test]
    fn webkit_without_an_isolated_profile_is_not_benchmarkable() {
        let capabilities = browser_capabilities(&installation(BrowserEngine::WebKit));
        assert!(!capabilities.isolated_profile);
        assert!(!capabilities.tab_creation);
    }
}
