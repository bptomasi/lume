//! Explicit local credentials serialize only for private configuration storage.

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// A locally stored API token whose debug representation never exposes bytes.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ApiToken(String);

impl ApiToken {
    /// Expose the credential only at its storage or authenticated HTTP boundary.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Reject empty, excessive or header-invalid credentials without echoing them.
    ///
    /// # Errors
    /// Returns a bounded validation error containing no credential value.
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.0.trim().is_empty()
                && self.0.len() <= 4096
                && !self.0.chars().any(char::is_control),
            "API token must contain 1–4,096 bytes without control characters"
        );
        Ok(())
    }
}

impl From<String> for ApiToken {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for ApiToken {
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

impl std::fmt::Debug for ApiToken {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        output.write_str("ApiToken([redacted])")
    }
}
