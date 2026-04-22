mod inspect;
mod sanitize;
mod write;

use std::path::PathBuf;

use agent_matters_core::domain::{Diagnostic, DiagnosticLocation, DiagnosticSeverity};
use serde::Serialize;
use thiserror::Error;

use crate::catalog::{
    CatalogIndexError, CatalogIndexStatus, LoadCatalogIndexRequest, discover_catalog,
    load_or_refresh_catalog_index,
};

use self::inspect::inspect_runtime_home;
use self::write::{detect_conflicts, write_runtime_import_plan};

#[derive(Debug, Clone)]
pub struct ImportRuntimeHomeRequest {
    pub repo_root: PathBuf,
    pub user_state_dir: PathBuf,
    pub runtime: Option<String>,
    pub source_home: PathBuf,
    pub profile: Option<String>,
    pub write: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeHomeImportStatus {
    DryRun,
    Imported,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeHomeImportResult {
    pub status: RuntimeHomeImportStatus,
    pub runtime: String,
    pub source_home: PathBuf,
    pub profile_id: String,
    pub profile_manifest_path: PathBuf,
    pub capabilities: Vec<RuntimeHomeImportCapability>,
    pub skipped_files: Vec<RuntimeHomeImportSkippedFile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index_status: Option<CatalogIndexStatus>,
    pub diagnostics: Vec<Diagnostic>,
}

impl RuntimeHomeImportResult {
    pub fn has_error_diagnostics(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeHomeImportCapability {
    pub id: String,
    pub kind: String,
    pub source_path: String,
    pub manifest_path: PathBuf,
    pub vendor_path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeHomeImportSkippedFile {
    pub path: PathBuf,
    pub reason: String,
}

#[derive(Debug, Error)]
pub enum RuntimeHomeImportError {
    #[error("failed to serialize manifest `{id}`: {source}")]
    SerializeManifest {
        id: String,
        source: toml::ser::Error,
    },
    #[error("failed to serialize vendor record `{id}`: {source}")]
    SerializeVendorRecord {
        id: String,
        source: serde_json::Error,
    },
    #[error("failed to write `{path}`: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error(transparent)]
    Index(#[from] CatalogIndexError),
}

impl RuntimeHomeImportError {
    pub fn to_diagnostic(&self) -> Diagnostic {
        match self {
            Self::SerializeManifest { id, source } => Diagnostic::new(
                DiagnosticSeverity::Error,
                "source.runtime-import-manifest-serialize-failed",
                format!("failed to serialize imported manifest `{id}`: {source}"),
            ),
            Self::SerializeVendorRecord { id, source } => Diagnostic::new(
                DiagnosticSeverity::Error,
                "source.runtime-import-vendor-serialize-failed",
                format!("failed to serialize imported vendor record `{id}`: {source}"),
            ),
            Self::Write { path, source } => Diagnostic::new(
                DiagnosticSeverity::Error,
                "source.runtime-import-write-failed",
                format!(
                    "failed to write runtime import path `{}`: {source}",
                    path.display()
                ),
            )
            .with_location(DiagnosticLocation::manifest_path(path)),
            Self::Index(source) => Diagnostic::new(
                DiagnosticSeverity::Error,
                "source.runtime-import-index-refresh-failed",
                format!("failed to refresh catalog index after runtime import: {source}"),
            )
            .with_recovery_hint("run `agent-matters profiles list` to rebuild the index"),
        }
    }
}

#[derive(Debug)]
pub(super) struct RuntimeImportPlan {
    pub runtime: String,
    pub source_home: PathBuf,
    pub profile_id: String,
    pub profile_manifest_path: PathBuf,
    pub profile: agent_matters_core::manifest::ProfileManifest,
    pub capabilities: Vec<PlannedCapability>,
    pub skipped_files: Vec<RuntimeHomeImportSkippedFile>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug)]
pub(super) struct PlannedCapability {
    pub manifest: agent_matters_core::manifest::CapabilityManifest,
    pub source_path: String,
    pub manifest_path: PathBuf,
    pub vendor_path: PathBuf,
    pub catalog_files: Vec<PlannedFile>,
    pub vendor_record: serde_json::Value,
}

#[derive(Debug)]
pub(super) struct PlannedFile {
    pub relative_path: PathBuf,
    pub contents: Vec<u8>,
}

pub fn import_runtime_home(
    request: ImportRuntimeHomeRequest,
) -> Result<RuntimeHomeImportResult, RuntimeHomeImportError> {
    let mut plan = inspect_runtime_home(&request);
    let mut discovery = discover_catalog(&request.repo_root);
    plan.diagnostics.append(&mut discovery.diagnostics);
    if !has_error_diagnostics(&plan.diagnostics) {
        plan.diagnostics
            .extend(detect_conflicts(&request.repo_root, &plan, &discovery));
    }

    let mut result = result_from_plan(RuntimeHomeImportStatus::DryRun, &plan);
    if !request.write || result.has_error_diagnostics() {
        return Ok(result);
    }

    write_runtime_import_plan(&request.repo_root, &plan)?;
    let loaded = load_or_refresh_catalog_index(LoadCatalogIndexRequest {
        repo_root: request.repo_root,
        user_state_dir: request.user_state_dir,
    })?;
    result.status = RuntimeHomeImportStatus::Imported;
    result.index_path = Some(loaded.index_path);
    result.index_status = Some(loaded.status);
    result.diagnostics.extend(loaded.diagnostics);
    Ok(result)
}

fn result_from_plan(
    status: RuntimeHomeImportStatus,
    plan: &RuntimeImportPlan,
) -> RuntimeHomeImportResult {
    RuntimeHomeImportResult {
        status,
        runtime: plan.runtime.clone(),
        source_home: plan.source_home.clone(),
        profile_id: plan.profile_id.clone(),
        profile_manifest_path: plan.profile_manifest_path.clone(),
        capabilities: plan
            .capabilities
            .iter()
            .map(|capability| RuntimeHomeImportCapability {
                id: capability.manifest.id.to_string(),
                kind: capability.manifest.kind.as_str().to_string(),
                source_path: capability.source_path.clone(),
                manifest_path: capability.manifest_path.clone(),
                vendor_path: capability.vendor_path.clone(),
            })
            .collect(),
        skipped_files: plan.skipped_files.clone(),
        index_path: None,
        index_status: None,
        diagnostics: plan.diagnostics.clone(),
    }
}

fn has_error_diagnostics(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
}
