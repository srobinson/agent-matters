//! Source import use case dispatch and catalog persistence.

use std::path::PathBuf;

use agent_matters_core::config::SourceTrustPolicy;
use agent_matters_core::domain::{
    CapabilityKind, Diagnostic, DiagnosticLocation, DiagnosticSeverity,
};
use serde::Serialize;
use thiserror::Error;

use crate::catalog::{
    CatalogIndexError, CatalogIndexStatus, LoadCatalogIndexRequest, load_or_refresh_catalog_index,
};
use crate::config::{ConfigError, load_effective_source_trust_policy};

use super::{
    SkillsShAdapter, SourceAdapter, SourceAdapterError, SourceImportRequest,
    SourceImportStorageError, WriteSourceImportRequest, write_source_import,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSourceRequest {
    pub repo_root: PathBuf,
    pub user_state_dir: PathBuf,
    pub locator: String,
    pub replace_existing: bool,
}

pub struct ImportSourceAdapterRequest<'a, A: SourceAdapter> {
    pub repo_root: PathBuf,
    pub user_state_dir: PathBuf,
    pub locator: String,
    pub replace_existing: bool,
    pub adapter: &'a A,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImportSourceResult {
    pub source: String,
    pub locator: String,
    pub capability_id: String,
    pub manifest_path: PathBuf,
    pub vendor_dir: PathBuf,
    pub index_path: PathBuf,
    pub index_status: CatalogIndexStatus,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Error)]
pub enum ImportSourceError {
    #[error("source locator `{locator}` must use `<source>:<locator>`")]
    InvalidLocator { locator: String },
    #[error("source `{source_id}` is blocked from importing capability kind `{kind}`")]
    TrustBlocked { source_id: String, kind: String },
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Adapter(#[from] SourceAdapterError),
    #[error(transparent)]
    Storage(#[from] SourceImportStorageError),
    #[error(transparent)]
    Index(#[from] CatalogIndexError),
}

impl ImportSourceError {
    pub fn to_diagnostic(&self) -> Diagnostic {
        match self {
            Self::InvalidLocator { locator } => Diagnostic::new(
                DiagnosticSeverity::Error,
                "source.locator-invalid",
                format!("source locator `{locator}` must use `<source>:<locator>`"),
            )
            .with_recovery_hint(
                "try `<source>:<locator>`, for example `skills.sh:owner/repo@skill-name`",
            ),
            Self::TrustBlocked { source_id, kind } => Diagnostic::new(
                DiagnosticSeverity::Error,
                "source.trust-blocked",
                format!(
                    "source `{source_id}` is blocked from importing capability kind `{kind}` by policy"
                ),
            )
            .with_recovery_hint(
                "allow this source and kind in defaults/sources.toml or user config.toml",
            ),
            Self::Config(ConfigError::Io { path, source }) => Diagnostic::new(
                DiagnosticSeverity::Error,
                "source.config-read-failed",
                format!(
                    "failed to read source trust config `{}`: {source}",
                    path.display()
                ),
            )
            .with_location(DiagnosticLocation::manifest_path(path)),
            Self::Config(ConfigError::Parse { path, source }) => Diagnostic::new(
                DiagnosticSeverity::Error,
                "source.config-parse-failed",
                format!(
                    "failed to parse source trust config `{}`: {source}",
                    path.display()
                ),
            )
            .with_location(DiagnosticLocation::manifest_path(path)),
            Self::Adapter(source) => source.to_diagnostic(),
            Self::Storage(SourceImportStorageError::AlreadyExists { path }) => Diagnostic::new(
                DiagnosticSeverity::Error,
                "source.import-conflict",
                format!("source import target `{}` already exists", path.display()),
            )
            .with_recovery_hint(
                "remove the existing local capability or choose a different source locator",
            ),
            Self::Storage(source) => Diagnostic::new(
                DiagnosticSeverity::Error,
                "source.import-write-failed",
                format!("failed to write source import: {source}"),
            )
            .with_recovery_hint("inspect the catalog and vendor paths, then retry the import"),
            Self::Index(source) => Diagnostic::new(
                DiagnosticSeverity::Error,
                "source.index-refresh-failed",
                format!("failed to refresh catalog index after import: {source}"),
            )
            .with_recovery_hint("run `agent-matters capabilities list` to rebuild the index"),
        }
    }
}

pub fn import_source(
    request: ImportSourceRequest,
) -> Result<ImportSourceResult, ImportSourceError> {
    let (source, locator) = split_source_locator(&request.locator)?;
    let policy = load_effective_source_trust_policy(&request.repo_root, &request.user_state_dir)?;
    if !policy.allows_source(&source) {
        return Err(trust_blocked(source, None));
    }

    match source.as_str() {
        "skills.sh" => import_source_from_adapter_with_policy(
            ImportSourceAdapterRequest {
                repo_root: request.repo_root,
                user_state_dir: request.user_state_dir,
                locator,
                replace_existing: request.replace_existing,
                adapter: &SkillsShAdapter::default(),
            },
            &policy,
        ),
        other => Err(SourceAdapterError::import_failed(
            other,
            locator,
            "unsupported source; supported sources: skills.sh",
        )
        .into()),
    }
}

pub fn import_source_from_adapter<A: SourceAdapter>(
    request: ImportSourceAdapterRequest<'_, A>,
) -> Result<ImportSourceResult, ImportSourceError> {
    import_source_from_adapter_with_policy(request, &SourceTrustPolicy::conservative_default())
}

pub fn import_source_from_adapter_with_policy<A: SourceAdapter>(
    request: ImportSourceAdapterRequest<'_, A>,
    policy: &SourceTrustPolicy,
) -> Result<ImportSourceResult, ImportSourceError> {
    let imported = request.adapter.import_capability(SourceImportRequest {
        locator: request.locator,
    })?;
    let kind = imported.manifest.kind;
    if !policy.allows_import(&imported.source, kind) {
        return Err(trust_blocked(imported.source, Some(kind)));
    }

    let capability_id = imported.manifest.id.to_string();
    let source = imported.source.clone();
    let locator = imported.locator.clone();
    let written = write_source_import(WriteSourceImportRequest {
        repo_root: request.repo_root.clone(),
        import: imported,
        replace_existing: request.replace_existing,
    })?;
    let loaded = load_or_refresh_catalog_index(LoadCatalogIndexRequest {
        repo_root: request.repo_root,
        user_state_dir: request.user_state_dir,
    })?;
    let mut diagnostics = written.diagnostics;
    diagnostics.extend(loaded.diagnostics);

    Ok(ImportSourceResult {
        source,
        locator,
        capability_id,
        manifest_path: written.manifest_path,
        vendor_dir: written.vendor_dir,
        index_path: loaded.index_path,
        index_status: loaded.status,
        diagnostics,
    })
}

fn trust_blocked(source: String, kind: Option<CapabilityKind>) -> ImportSourceError {
    ImportSourceError::TrustBlocked {
        source_id: source,
        kind: kind
            .map(|kind| kind.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
    }
}

fn split_source_locator(locator: &str) -> Result<(String, String), ImportSourceError> {
    let Some((source, rest)) = locator.split_once(':') else {
        return Err(ImportSourceError::InvalidLocator {
            locator: locator.to_string(),
        });
    };
    if source.trim().is_empty() || rest.trim().is_empty() {
        return Err(ImportSourceError::InvalidLocator {
            locator: locator.to_string(),
        });
    }

    Ok((source.trim().to_string(), rest.trim().to_string()))
}
