//! Structured diagnostics shared by validation and doctor workflows.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

use serde::{Deserialize, Serialize, Serializer};

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

/// JSON projection for commands that only need to return diagnostics.
#[derive(Debug, Serialize)]
pub struct DiagnosticReport<'a> {
    pub diagnostics: &'a [Diagnostic],
}

impl<'a> DiagnosticReport<'a> {
    pub fn new(diagnostics: &'a [Diagnostic]) -> Self {
        Self { diagnostics }
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

impl DiagnosticSeverity {
    pub const ORDERED: [Self; 3] = [Self::Error, Self::Warning, Self::Info];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }

    pub fn heading(self) -> &'static str {
        match self {
            Self::Error => "Errors",
            Self::Warning => "Warnings",
            Self::Info => "Info",
        }
    }
}

/// Optional location for a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticLocation {
    #[serde(
        default,
        rename = "source_path",
        serialize_with = "serialize_optional_path",
        skip_serializing_if = "Option::is_none"
    )]
    pub manifest_path: Option<PathBuf>,
    #[serde(
        default,
        rename = "field_path",
        skip_serializing_if = "Option::is_none"
    )]
    pub field: Option<String>,
}

impl DiagnosticLocation {
    pub fn source_path(path: impl Into<PathBuf>) -> Self {
        Self::manifest_path(path)
    }

    pub fn manifest_path(path: impl Into<PathBuf>) -> Self {
        Self {
            manifest_path: Some(path.into()),
            field: None,
        }
    }

    pub fn source_field(path: impl Into<PathBuf>, field: impl Into<String>) -> Self {
        Self::manifest_field(path, field)
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

pub fn render_diagnostics_human(diagnostics: &[Diagnostic]) -> String {
    let mut rendered = String::new();

    for severity in DiagnosticSeverity::ORDERED {
        let grouped = diagnostics_by_source(diagnostics, severity);
        if grouped.is_empty() {
            continue;
        }
        if !rendered.is_empty() {
            rendered.push('\n');
        }
        let _ = writeln!(rendered, "{}:", severity.heading());
        for (source, diagnostics) in grouped {
            let _ = writeln!(rendered, "  {source}");
            for diagnostic in diagnostics {
                let _ = writeln!(rendered, "    {}: {}", diagnostic.code, diagnostic.message);
                if let Some(location) = &diagnostic.location
                    && let Some(field) = &location.field
                {
                    let _ = writeln!(rendered, "      field: {field}");
                }
                if let Some(hint) = &diagnostic.recovery_hint {
                    let _ = writeln!(rendered, "      hint: {hint}");
                }
            }
        }
    }

    rendered
}

fn diagnostics_by_source(
    diagnostics: &[Diagnostic],
    severity: DiagnosticSeverity,
) -> BTreeMap<String, Vec<&Diagnostic>> {
    let mut grouped = BTreeMap::<String, Vec<&Diagnostic>>::new();
    for diagnostic in diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == severity)
    {
        let source = diagnostic
            .location
            .as_ref()
            .and_then(|location| location.manifest_path.as_ref())
            .map(|path| normalize_path(path.as_path()))
            .unwrap_or_else(|| "<general>".to_string());
        grouped.entry(source).or_default().push(diagnostic);
    }
    grouped
}

fn serialize_optional_path<S>(path: &Option<PathBuf>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match path {
        Some(path) => serializer.serialize_some(&normalize_path(path)),
        None => serializer.serialize_none(),
    }
}

fn normalize_path(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
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
                    "source_path": "profiles/github-researcher/manifest.toml",
                    "field_path": "runtimes.codex.enabled"
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

    #[test]
    fn diagnostic_report_json_shape_is_structured() {
        let diagnostics = vec![
            Diagnostic::new(
                DiagnosticSeverity::Warning,
                "runtime.credential-source-missing",
                "codex credential source is missing",
            )
            .with_location(DiagnosticLocation::source_path(
                r"catalog\skills\playwright\manifest.toml",
            ))
            .with_recovery_hint("authenticate with the native runtime"),
        ];

        let encoded = serde_json::to_value(DiagnosticReport::new(&diagnostics)).unwrap();

        assert_eq!(
            encoded,
            json!({
                "diagnostics": [
                    {
                        "severity": "warning",
                        "code": "runtime.credential-source-missing",
                        "message": "codex credential source is missing",
                        "location": {
                            "source_path": "catalog/skills/playwright/manifest.toml"
                        },
                        "recovery_hint": "authenticate with the native runtime"
                    }
                ]
            })
        );
    }

    #[test]
    fn diagnostic_human_render_groups_by_severity_and_path() {
        let diagnostics = vec![
            Diagnostic::new(
                DiagnosticSeverity::Warning,
                "runtime.credential-source-missing",
                "codex credential source is missing",
            )
            .with_location(DiagnosticLocation::source_path(
                r"catalog\skills\playwright\manifest.toml",
            ))
            .with_recovery_hint("authenticate with the native runtime"),
            Diagnostic::new(
                DiagnosticSeverity::Error,
                "catalog.manifest-invalid",
                "manifest TOML could not be parsed",
            )
            .with_location(DiagnosticLocation::source_field(
                "catalog/mcp/bad-toml/manifest.toml",
                "runtimes.codex",
            )),
            Diagnostic::new(
                DiagnosticSeverity::Info,
                "doctor.index-stale",
                "generated index is older than the catalog",
            ),
        ];

        let rendered = render_diagnostics_human(&diagnostics);

        assert_eq!(
            rendered,
            "Errors:\n  catalog/mcp/bad-toml/manifest.toml\n    catalog.manifest-invalid: manifest TOML could not be parsed\n      field: runtimes.codex\n\nWarnings:\n  catalog/skills/playwright/manifest.toml\n    runtime.credential-source-missing: codex credential source is missing\n      hint: authenticate with the native runtime\n\nInfo:\n  <general>\n    doctor.index-stale: generated index is older than the catalog\n"
        );
    }
}
