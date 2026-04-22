use std::path::PathBuf;

use agent_matters_core::domain::{Diagnostic, DiagnosticSeverity};
use agent_matters_core::manifest::CapabilityManifest;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Adapter boundary for one external catalog source.
pub trait SourceAdapter {
    /// Stable source identifier used in provenance and vendor paths.
    fn source_id(&self) -> &str;

    /// Search the source and return source specific records in a normalized
    /// envelope. Raw records stay attached so callers can inspect what the
    /// adapter consumed.
    fn search(
        &self,
        request: SourceSearchRequest,
    ) -> Result<SourceSearchResult, SourceAdapterError>;

    /// Import one source locator into normalized catalog material plus raw
    /// vendor files. This does not write to disk.
    fn import_capability(
        &self,
        request: SourceImportRequest,
    ) -> Result<SourceImportResult, SourceAdapterError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSearchRequest {
    pub query: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceSearchResult {
    pub source: String,
    pub query: String,
    pub entries: Vec<SourceSearchEntry>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceSearchEntry {
    pub locator: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub raw: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceImportRequest {
    pub locator: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceImportResult {
    pub source: String,
    pub locator: String,
    pub manifest: CapabilityManifest,
    pub catalog_files: Vec<SourceImportFile>,
    pub vendor_files: Vec<SourceImportFile>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceImportFile {
    pub relative_path: PathBuf,
    pub contents: String,
}

#[derive(Debug, Error)]
pub enum SourceAdapterError {
    #[error("source `{source_id}` search failed: {message}")]
    SearchFailed { source_id: String, message: String },
    #[error("source `{source_id}` import failed for `{locator}`: {message}")]
    ImportFailed {
        source_id: String,
        locator: String,
        message: String,
    },
    #[error("source `{source_id}` returned invalid record `{locator}`: {message}")]
    InvalidRecord {
        source_id: String,
        locator: String,
        message: String,
    },
}

impl SourceAdapterError {
    pub fn search_failed(source: impl Into<String>, message: impl Into<String>) -> Self {
        Self::SearchFailed {
            source_id: source.into(),
            message: message.into(),
        }
    }

    pub fn import_failed(
        source: impl Into<String>,
        locator: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::ImportFailed {
            source_id: source.into(),
            locator: locator.into(),
            message: message.into(),
        }
    }

    pub fn invalid_record(
        source: impl Into<String>,
        locator: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::InvalidRecord {
            source_id: source.into(),
            locator: locator.into(),
            message: message.into(),
        }
    }

    pub fn to_diagnostic(&self) -> Diagnostic {
        match self {
            Self::SearchFailed { source_id, message } => Diagnostic::new(
                DiagnosticSeverity::Error,
                "source.search-failed",
                format!("source `{source_id}` search failed: {message}"),
            )
            .with_recovery_hint("inspect the source adapter command and try the search again"),
            Self::ImportFailed {
                source_id,
                locator,
                message,
            } => Diagnostic::new(
                DiagnosticSeverity::Error,
                "source.import-failed",
                format!("source `{source_id}` import failed for `{locator}`: {message}"),
            )
            .with_recovery_hint("inspect the source adapter command and import locator"),
            Self::InvalidRecord {
                source_id,
                locator,
                message,
            } => Diagnostic::new(
                DiagnosticSeverity::Error,
                "source.record-invalid",
                format!("source `{source_id}` returned invalid record `{locator}`: {message}"),
            )
            .with_recovery_hint("update the source adapter or skip this upstream record"),
        }
    }
}
