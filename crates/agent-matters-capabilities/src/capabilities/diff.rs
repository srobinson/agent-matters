//! Capability overlay diff use case.

use std::io;
use std::path::{Path, PathBuf};

use agent_matters_core::catalog::path_is_in_repo_vendor;
use agent_matters_core::domain::{Diagnostic, DiagnosticLocation, DiagnosticSeverity};
use serde::Serialize;

use crate::catalog::{CatalogIndexError, LoadCatalogIndexRequest, load_or_refresh_catalog_index};

use super::diff_tree::diff_directories;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffCapabilityRequest {
    pub repo_root: PathBuf,
    pub user_state_dir: PathBuf,
    pub capability: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiffCapabilityResult {
    pub capability: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlay_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor_path: Option<String>,
    pub files: Vec<CapabilityDiffFile>,
    pub diagnostics: Vec<Diagnostic>,
}

impl DiffCapabilityResult {
    pub fn has_error_diagnostics(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityDiffFile {
    pub path: String,
    pub status: CapabilityDiffStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlay_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CapabilityDiffStatus {
    Added,
    Removed,
    Changed,
}

#[derive(Debug, thiserror::Error)]
pub enum CapabilityDiffError {
    #[error(transparent)]
    CatalogIndex(#[from] CatalogIndexError),
    #[error("failed to read directory `{path}`: {source}")]
    ReadDirectory {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to read file `{path}`: {source}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to inspect file `{path}`: {source}")]
    InspectFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

pub fn diff_capability(
    request: DiffCapabilityRequest,
) -> Result<DiffCapabilityResult, CapabilityDiffError> {
    let loaded = load_or_refresh_catalog_index(LoadCatalogIndexRequest {
        repo_root: request.repo_root.clone(),
        user_state_dir: request.user_state_dir,
    })?;
    let mut result = DiffCapabilityResult {
        capability: request.capability.clone(),
        base_path: None,
        overlay_path: None,
        vendor_path: None,
        files: Vec::new(),
        diagnostics: loaded.diagnostics,
    };

    let Some(record) = loaded.index.capability(&request.capability) else {
        result.diagnostics.push(not_found(&request.capability));
        return Ok(result);
    };

    result.base_path = record.source.normalized_path.clone();
    result.overlay_path = record.source.overlay_path.clone();
    result.vendor_path = record.source.vendor_path.clone();

    if record.source.kind == "local" || result.vendor_path.is_none() {
        result
            .diagnostics
            .push(not_imported(&request.capability, &record.source_path));
        return Ok(result);
    }

    if result.overlay_path.is_none() {
        result
            .diagnostics
            .push(no_overlay(&request.capability, &record.source_path));
        return Ok(result);
    }

    let Some(base_path) = result.base_path.as_deref() else {
        result.diagnostics.push(missing_vendor_source(
            &request.capability,
            "normalized source path is missing from the generated index",
            None,
        ));
        return Ok(result);
    };
    let Some(overlay_path) = result.overlay_path.as_deref() else {
        unreachable!("overlay path was checked above");
    };
    let Some(vendor_path) = result.vendor_path.as_deref() else {
        unreachable!("vendor path was checked above");
    };

    let base = request.repo_root.join(base_path);
    let overlay = request.repo_root.join(overlay_path);
    let vendor = request.repo_root.join(vendor_path);

    if !path_is_in_repo_vendor(&request.repo_root, &vendor) {
        result
            .diagnostics
            .push(invalid_vendor_path(&request.capability, &vendor));
        return Ok(result);
    }
    if !vendor.exists() {
        result.diagnostics.push(missing_vendor_source(
            &request.capability,
            "vendor source directory is missing",
            Some(&vendor),
        ));
        return Ok(result);
    }
    if !base.exists() {
        result.diagnostics.push(missing_vendor_source(
            &request.capability,
            "normalized upstream source directory is missing",
            Some(&base),
        ));
        return Ok(result);
    }
    if !overlay.exists() {
        result
            .diagnostics
            .push(overlay_directory_missing(&request.capability, &overlay));
        return Ok(result);
    }

    result.files = diff_directories(&base, &overlay)?;
    Ok(result)
}

pub(super) fn diff_file(
    path: String,
    status: CapabilityDiffStatus,
    base_bytes: Option<u64>,
    overlay_bytes: Option<u64>,
) -> CapabilityDiffFile {
    let note = base_bytes
        .into_iter()
        .chain(overlay_bytes)
        .any(|bytes| bytes > 65_536)
        .then(|| "content diff omitted because file exceeds 65536 bytes".to_string());
    CapabilityDiffFile {
        path,
        status,
        base_bytes,
        overlay_bytes,
        note,
    }
}

fn not_found(capability: &str) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "capability.diff-not-found",
        format!("capability `{capability}` was not found in the generated catalog index"),
    )
    .with_location(DiagnosticLocation::field("capability"))
}

fn not_imported(capability: &str, path: &str) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "capability.diff-not-imported",
        format!("capability `{capability}` is not imported from an external source"),
    )
    .with_location(DiagnosticLocation::manifest_path(path))
    .with_recovery_hint("diff only imported capabilities with preserved vendor source")
}

fn no_overlay(capability: &str, path: &str) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "capability.diff-no-overlay",
        format!("capability `{capability}` has no local overlay to diff"),
    )
    .with_location(DiagnosticLocation::manifest_path(path))
    .with_recovery_hint("create a full copy overlay under overlays/<kind>/<name>")
}

fn invalid_vendor_path(capability: &str, path: &Path) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "capability.diff-vendor-path-invalid",
        format!(
            "capability `{capability}` cannot be diffed because vendor source path resolves outside repository vendor storage"
        ),
    )
    .with_location(DiagnosticLocation::manifest_path(path))
    .with_recovery_hint(
        "refresh the generated index or use relative origin source and locator values inside the vendor directory",
    )
}

fn missing_vendor_source(capability: &str, detail: &str, path: Option<&Path>) -> Diagnostic {
    let diagnostic = Diagnostic::new(
        DiagnosticSeverity::Error,
        "capability.diff-vendor-source-missing",
        format!("capability `{capability}` cannot be diffed because {detail}"),
    )
    .with_recovery_hint("restore the imported vendor source or reimport the capability");
    match path {
        Some(path) => diagnostic.with_location(DiagnosticLocation::manifest_path(path)),
        None => diagnostic,
    }
}

fn overlay_directory_missing(capability: &str, path: &Path) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "capability.diff-overlay-missing",
        format!("capability `{capability}` overlay directory is missing"),
    )
    .with_location(DiagnosticLocation::manifest_path(path))
    .with_recovery_hint("restore the overlay directory or refresh the generated index")
}
