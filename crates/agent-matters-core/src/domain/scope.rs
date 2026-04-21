//! Scope constraints for profile activation and use.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Path and repository constraints declared by a profile.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeConstraints {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub github_repos: Vec<String>,
    #[serde(default, skip_serializing_if = "ScopeEnforcement::is_none")]
    pub enforcement: ScopeEnforcement,
}

/// How profile scope violations are handled.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScopeEnforcement {
    #[default]
    None,
    Warn,
    Fail,
}

impl ScopeEnforcement {
    pub const fn as_str(&self) -> &'static str {
        match self {
            ScopeEnforcement::None => "none",
            ScopeEnforcement::Warn => "warn",
            ScopeEnforcement::Fail => "fail",
        }
    }

    pub const fn all() -> &'static [ScopeEnforcement] {
        &[
            ScopeEnforcement::None,
            ScopeEnforcement::Warn,
            ScopeEnforcement::Fail,
        ]
    }

    pub const fn is_none(&self) -> bool {
        matches!(self, ScopeEnforcement::None)
    }
}

impl fmt::Display for ScopeEnforcement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ScopeEnforcement {
    type Err = ScopeEnforcementError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        for mode in ScopeEnforcement::all() {
            if mode.as_str() == s {
                return Ok(*mode);
            }
        }
        Err(ScopeEnforcementError::Unknown {
            mode: s.to_string(),
        })
    }
}

/// Error produced when parsing a scope enforcement mode.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScopeEnforcementError {
    #[error("scope enforcement `{mode}` is not recognized; expected one of: none, warn, fail")]
    Unknown { mode: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_enforcement_modes_parse_strictly() {
        assert_eq!("none".parse(), Ok(ScopeEnforcement::None));
        assert_eq!("warn".parse(), Ok(ScopeEnforcement::Warn));
        assert_eq!("fail".parse(), Ok(ScopeEnforcement::Fail));

        let err = "warning".parse::<ScopeEnforcement>().unwrap_err();
        assert!(err.to_string().contains("warning"));
    }

    #[test]
    fn scope_constraints_parse_manifest_shape() {
        let src = r#"
            paths = ["~/Dev/LLM/DEV/helioy"]
            github_repos = ["srobinson/helioy"]
            enforcement = "warn"
        "#;

        let scope: ScopeConstraints = toml::from_str(src).unwrap();

        assert_eq!(scope.paths, vec!["~/Dev/LLM/DEV/helioy".to_string()]);
        assert_eq!(scope.github_repos, vec!["srobinson/helioy".to_string()]);
        assert_eq!(scope.enforcement, ScopeEnforcement::Warn);
    }

    #[test]
    fn scope_enforcement_defaults_to_none() {
        let scope: ScopeConstraints = toml::from_str(r#"paths = ["/tmp"]"#).unwrap();
        let encoded = serde_json::to_string(&scope).unwrap();

        assert_eq!(scope.enforcement, ScopeEnforcement::None);
        assert!(!encoded.contains("enforcement"));
    }

    #[test]
    fn unknown_scope_enforcement_is_rejected_by_serde() {
        let err = toml::from_str::<ScopeConstraints>(r#"enforcement = "warning""#).unwrap_err();

        assert!(err.to_string().contains("warning"));
    }
}
