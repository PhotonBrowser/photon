use std::{collections::BTreeMap, fmt, path::PathBuf};

use crate::model::{BrowserId, BrowserInstallation, IdentifierError, InstallationId};

/// Platform-independent registry of discovered browser installations.
#[derive(Clone, Debug, Default)]
pub struct BrowserRegistry {
    installations: BTreeMap<InstallationId, BrowserInstallation>,
}

impl BrowserRegistry {
    /// Adds an installation. Repeated discovery of the same installation is idempotent.
    pub fn insert(&mut self, installation: BrowserInstallation) -> Result<(), RegistryError> {
        if !installation.executable_path.is_file() {
            return Err(RegistryError::MissingExecutable(
                installation.executable_path,
            ));
        }

        if let Some(existing) = self.installations.get(&installation.installation_id) {
            if existing.executable_path == installation.executable_path {
                return Ok(());
            }
            return Err(RegistryError::DuplicateInstallation(
                installation.installation_id,
            ));
        }

        self.installations
            .insert(installation.installation_id.clone(), installation);
        Ok(())
    }

    pub fn installations(&self) -> impl Iterator<Item = &BrowserInstallation> {
        self.installations.values()
    }

    /// Resolves an installation ID or an unambiguous browser family ID.
    pub fn resolve(&self, selector: &str) -> Result<&BrowserInstallation, RegistryError> {
        let parsed =
            InstallationId::new(selector.to_owned()).map_err(RegistryError::InvalidIdentifier)?;
        if let Some(installation) = self.installations.get(&parsed) {
            return Ok(installation);
        }

        let browser_id =
            BrowserId::new(selector.to_owned()).map_err(RegistryError::InvalidIdentifier)?;
        let matches = self
            .installations
            .values()
            .filter(|installation| installation.metadata.id == browser_id)
            .collect::<Vec<_>>();

        match matches.as_slice() {
            [installation] => Ok(installation),
            [] => Err(RegistryError::Unavailable(browser_id)),
            _ => Err(RegistryError::Ambiguous {
                browser_id,
                installations: matches
                    .iter()
                    .map(|installation| installation.installation_id.clone())
                    .collect(),
            }),
        }
    }

    pub fn by_browser(&self, browser_id: &BrowserId) -> Vec<&BrowserInstallation> {
        self.installations
            .values()
            .filter(|installation| &installation.metadata.id == browser_id)
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistryError {
    InvalidIdentifier(IdentifierError),
    Unavailable(BrowserId),
    Ambiguous {
        browser_id: BrowserId,
        installations: Vec<InstallationId>,
    },
    MissingExecutable(PathBuf),
    DuplicateInstallation(InstallationId),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentifier(error) => error.fmt(formatter),
            Self::Unavailable(browser_id) => {
                write!(formatter, "browser '{browser_id}' is not available")
            }
            Self::Ambiguous {
                browser_id,
                installations,
            } => write!(
                formatter,
                "browser '{browser_id}' has multiple installations; select one of: {}",
                installations
                    .iter()
                    .map(InstallationId::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::MissingExecutable(path) => {
                write!(
                    formatter,
                    "browser executable is missing: {}",
                    path.display()
                )
            }
            Self::DuplicateInstallation(id) => {
                write!(
                    formatter,
                    "installation identifier '{id}' is already in use"
                )
            }
        }
    }
}

impl std::error::Error for RegistryError {}

#[cfg(test)]
mod tests {
    use std::{fs, time::SystemTime};

    use super::*;
    use crate::model::{BrowserEngine, BrowserMetadata};

    fn fixture_installation(
        installation_id: &str,
        executable_path: PathBuf,
    ) -> BrowserInstallation {
        BrowserInstallation {
            installation_id: InstallationId::new(installation_id).unwrap(),
            metadata: BrowserMetadata {
                id: BrowserId::new("chrome").unwrap(),
                name: "Google Chrome".to_owned(),
                bundle_identifier: Some("com.google.Chrome".to_owned()),
                engine: Some(BrowserEngine::Chromium),
            },
            version: Some("1.0".to_owned()),
            application_path: executable_path.clone(),
            executable_path,
        }
    }

    fn executable_fixture() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("velocity-registry-{unique}"));
        fs::write(&path, b"fixture").unwrap();
        path
    }

    #[test]
    fn allows_and_resolves_multiple_installations_of_one_browser() {
        let first_path = executable_fixture();
        let second_path = executable_fixture();
        let mut registry = BrowserRegistry::default();
        registry
            .insert(fixture_installation("chrome-work", first_path.clone()))
            .unwrap();
        registry
            .insert(fixture_installation("chrome-personal", second_path.clone()))
            .unwrap();

        assert!(matches!(
            registry.resolve("chrome"),
            Err(RegistryError::Ambiguous { .. })
        ));
        assert_eq!(
            registry.resolve("chrome-work").unwrap().executable_path,
            first_path
        );
        assert_eq!(
            registry
                .by_browser(&BrowserId::new("chrome").unwrap())
                .len(),
            2
        );

        fs::remove_file(first_path).unwrap();
        fs::remove_file(second_path).unwrap();
    }

    #[test]
    fn repeated_discovery_of_the_same_installation_is_idempotent() {
        let path = executable_fixture();
        let installation = fixture_installation("chrome-main", path.clone());
        let mut registry = BrowserRegistry::default();
        registry.insert(installation.clone()).unwrap();
        registry.insert(installation).unwrap();
        assert_eq!(registry.installations().count(), 1);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_missing_executables_and_invalid_identifiers() {
        let missing = std::env::temp_dir().join("velocity-missing-executable");
        let mut registry = BrowserRegistry::default();
        assert!(matches!(
            registry.insert(fixture_installation("chrome-main", missing.clone())),
            Err(RegistryError::MissingExecutable(path)) if path == missing
        ));
        assert!(matches!(
            registry.resolve("Chrome!"),
            Err(RegistryError::InvalidIdentifier(_))
        ));
        assert!(matches!(
            BrowserId::new("not valid"),
            Err(IdentifierError(_))
        ));
    }
}
