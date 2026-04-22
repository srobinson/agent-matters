//! Capability and environment requirements declared by capabilities.
//!
//! The core model keeps requirement checks pure. Callers provide observed
//! environment state, and the check result never carries secret values.

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::CapabilityId;

/// Required capabilities and environment variables for a capability.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Requirements {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<CapabilityId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env: Vec<EnvVarRequirement>,
}

impl Requirements {
    /// Check required environment variables against a provided environment map.
    pub fn check_env(&self, env: &BTreeMap<String, String>) -> Vec<EnvVarCheck> {
        self.env
            .iter()
            .map(|requirement| requirement.check_presence(env.contains_key(requirement.name())))
            .collect()
    }
}

/// Name of a required environment variable.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EnvVarRequirement(String);

impl EnvVarRequirement {
    pub fn new(name: impl Into<String>) -> Result<Self, EnvVarRequirementError> {
        let name = name.into();
        if name.is_empty() {
            return Err(EnvVarRequirementError::Empty);
        }
        if name.contains('\0') {
            return Err(EnvVarRequirementError::ContainsNul);
        }
        if name.contains('=') {
            return Err(EnvVarRequirementError::ContainsEquals);
        }
        Ok(Self(name))
    }

    pub fn name(&self) -> &str {
        &self.0
    }

    pub fn check_presence(&self, present: bool) -> EnvVarCheck {
        EnvVarCheck {
            name: self.0.clone(),
            status: if present {
                EnvVarPresence::Present
            } else {
                EnvVarPresence::Missing
            },
        }
    }
}

impl fmt::Display for EnvVarRequirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for EnvVarRequirement {
    type Err = EnvVarRequirementError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        EnvVarRequirement::new(s)
    }
}

impl Serialize for EnvVarRequirement {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for EnvVarRequirement {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

/// Error produced when parsing a required environment variable name.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EnvVarRequirementError {
    #[error("environment variable requirement must not be empty")]
    Empty,
    #[error("environment variable requirement must not contain nul bytes")]
    ContainsNul,
    #[error("environment variable requirement must not contain `=`")]
    ContainsEquals,
}

/// Result of checking one required environment variable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvVarCheck {
    pub name: String,
    pub status: EnvVarPresence,
}

/// Presence status for a required environment variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EnvVarPresence {
    Present,
    Missing,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requirements_parse_env_and_capabilities() {
        let src = r#"
            env = ["LINEAR_API_KEY"]
            capabilities = ["mcp:context-matters"]
        "#;

        let requirements: Requirements = toml::from_str(src).unwrap();

        assert_eq!(requirements.env[0].name(), "LINEAR_API_KEY");
        assert_eq!(
            requirements.capabilities[0].to_string(),
            "mcp:context-matters"
        );
    }

    #[test]
    fn env_var_name_rejects_empty_and_equals() {
        assert_eq!(
            EnvVarRequirement::new(""),
            Err(EnvVarRequirementError::Empty)
        );
        assert!(matches!(
            EnvVarRequirement::new("TOKEN=value"),
            Err(EnvVarRequirementError::ContainsEquals)
        ));
        assert_eq!(
            EnvVarRequirement::new("TOKEN\0"),
            Err(EnvVarRequirementError::ContainsNul)
        );
    }

    #[test]
    fn env_var_assignment_error_does_not_echo_secret() {
        let err = EnvVarRequirement::new("TOKEN=secret-value").unwrap_err();
        let message = err.to_string();

        assert!(!message.contains("TOKEN"));
        assert!(!message.contains("secret-value"));
    }

    #[test]
    fn env_var_checks_report_presence_without_values() {
        let requirements: Requirements =
            toml::from_str(r#"env = ["LINEAR_API_KEY", "GH_TOKEN"]"#).unwrap();
        let env = BTreeMap::from([("LINEAR_API_KEY".to_string(), "secret-value".to_string())]);

        let checks = requirements.check_env(&env);
        let encoded = serde_json::to_string(&checks).unwrap();

        assert_eq!(checks[0].status, EnvVarPresence::Present);
        assert_eq!(checks[1].status, EnvVarPresence::Missing);
        assert!(encoded.contains("LINEAR_API_KEY"));
        assert!(!encoded.contains("secret-value"));
    }
}
