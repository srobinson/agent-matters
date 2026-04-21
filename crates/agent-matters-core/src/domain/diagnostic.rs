//! Structured diagnostics shared by validation and doctor workflows.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Machine readable diagnostic emitted by core validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<DiagnosticLocation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_hint: Option<String>,
}

impl Diagnostic {
    pub fn new(
        severity: DiagnosticSeverity,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
            location: None,
            recovery_hint: None,
        }
    }

    pub fn with_location(mut self, location: DiagnosticLocation) -> Self {
        self.location = Some(location);
        self
    }

    pub fn with_recovery_hint(mut self, recovery_hint: impl Into<String>) -> Self {
        self.recovery_hint = Some(recovery_hint.into());
        self
    }
}

/// Diagnostic severity levels used by human and JSON projections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

/// Optional location for a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticLocation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

impl DiagnosticLocation {
    pub fn manifest_path(path: impl Into<PathBuf>) -> Self {
        Self {
            manifest_path: Some(path.into()),
            field: None,
        }
    }

    pub fn manifest_field(path: impl Into<PathBuf>, field: impl Into<String>) -> Self {
        Self {
            manifest_path: Some(path.into()),
            field: Some(field.into()),
        }
    }

    pub fn field(field: impl Into<String>) -> Self {
        Self {
            manifest_path: None,
            field: Some(field.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn diagnostic_json_shape_is_structured() {
        let diagnostic = Diagnostic::new(
            DiagnosticSeverity::Error,
            "manifest.missing-field",
            "missing required runtime field",
        )
        .with_location(DiagnosticLocation::manifest_field(
            "profiles/github-researcher/manifest.toml",
            "runtimes.codex.enabled",
        ))
        .with_recovery_hint("add `enabled = true` under `[runtimes.codex]`");

        let encoded = serde_json::to_value(&diagnostic).unwrap();

        assert_eq!(
            encoded,
            json!({
                "severity": "error",
                "code": "manifest.missing-field",
                "message": "missing required runtime field",
                "location": {
                    "manifest_path": "profiles/github-researcher/manifest.toml",
                    "field": "runtimes.codex.enabled"
                },
                "recovery_hint": "add `enabled = true` under `[runtimes.codex]`"
            })
        );
    }

    #[test]
    fn diagnostic_omits_unknown_location_and_hint() {
        let diagnostic = Diagnostic::new(
            DiagnosticSeverity::Warning,
            "env.missing",
            "LINEAR_API_KEY is not set",
        );

        let encoded = serde_json::to_value(&diagnostic).unwrap();

        assert_eq!(
            encoded,
            json!({
                "severity": "warning",
                "code": "env.missing",
                "message": "LINEAR_API_KEY is not set"
            })
        );
    }
}
