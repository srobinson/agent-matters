//! Source import use case dispatch and catalog persistence.

use std::path::PathBuf;

use agent_matters_core::domain::{Diagnostic, DiagnosticSeverity};
use thiserror::Error;

use crate::catalog::{
    CatalogIndexError, CatalogIndexStatus, LoadCatalogIndexRequest, load_or_refresh_catalog_index,
};

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

#[derive(Debug, Clone, PartialEq, Eq)]
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
    match source.as_str() {
        "skills.sh" => import_source_from_adapter(ImportSourceAdapterRequest {
            repo_root: request.repo_root,
            user_state_dir: request.user_state_dir,
            locator,
            replace_existing: request.replace_existing,
            adapter: &SkillsShAdapter::default(),
        }),
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
    let imported = request.adapter.import_capability(SourceImportRequest {
        locator: request.locator,
    })?;
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
