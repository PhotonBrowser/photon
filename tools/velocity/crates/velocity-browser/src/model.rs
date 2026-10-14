use std::{fmt, path::PathBuf};

use serde::{Deserialize, Serialize};

/// A stable browser family identifier such as `chrome` or `safari`.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct BrowserId(String);

impl BrowserId {
    pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
        validate_identifier(value.into()).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BrowserId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A stable selector for one installation of a browser.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct InstallationId(String);

impl InstallationId {
    pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
        validate_identifier(value.into()).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for InstallationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

fn validate_identifier(value: String) -> Result<String, IdentifierError> {
    let valid = !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && value.as_bytes()[0].is_ascii_lowercase()
        && value.as_bytes()[value.len() - 1] != b'-';

    if valid {
        Ok(value)
    } else {
        Err(IdentifierError(value))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentifierError(pub String);

impl fmt::Display for IdentifierError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid identifier '{}'; use lowercase letters, digits, and interior hyphens",
            self.0
        )
    }
}

impl std::error::Error for IdentifierError {}

/// Browser engine family, when it is known from the browser's identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrowserEngine {
    Chromium,
    Gecko,
    WebKit,
    Ladybird,
}

/// Metadata shared by all installations of one browser family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserMetadata {
    pub id: BrowserId,
    pub name: String,
    pub bundle_identifier: Option<String>,
    pub engine: Option<BrowserEngine>,
}

/// One installed application or Photon checkout build.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserInstallation {
    pub installation_id: InstallationId,
    pub metadata: BrowserMetadata,
    pub version: Option<String>,
    pub application_path: PathBuf,
    pub executable_path: PathBuf,
}
