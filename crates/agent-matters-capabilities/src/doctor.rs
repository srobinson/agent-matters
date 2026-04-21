//! Doctor checks for authored catalog integrity and generated index state.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use agent_matters_core::catalog::CatalogIndex;
use agent_matters_core::domain::{Diagnostic, DiagnosticLocation, DiagnosticSeverity};
use serde::{Deserialize, Serialize};

use crate::catalog::{
    CatalogDiscovery, CatalogIndexError, DiscoveredCapabilityManifest, DiscoveredProfileManifest,
    build_catalog_index, catalog_index_path, discover_catalog,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorRequest {
    pub repo_root: PathBuf,
    pub user_state_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorResult {
    pub catalog: DoctorCatalogSummary,
    pub index: DoctorIndexSummary,
    pub diagnostics: Vec<Diagnostic>,
}

impl DoctorResult {
    pub fn has_error_diagnostics(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorCatalogSummary {
    pub capability_count: usize,
    pub profile_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorIndexSummary {
    pub path: PathBuf,
    pub status: DoctorIndexStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DoctorIndexStatus {
    Fresh,
    Missing,
    Stale,
    Corrupt,
    ReadFailed,
    Unchecked,
}

pub fn run_doctor(request: DoctorRequest) -> Result<DoctorResult, CatalogIndexError> {
    let discovery = discover_catalog(&request.repo_root);
    let mut diagnostics = discovery.diagnostics.clone();
    let capability_ids = unique_capability_ids(&discovery);

    validate_capability_files(&discovery, &mut diagnostics);
    validate_capability_requirements(&discovery, &capability_ids, &mut diagnostics);
    validate_profile_references(&discovery, &capability_ids, &mut diagnostics);

    let current_index = match build_catalog_index(&request.repo_root, &discovery) {
        Ok(index) => Some(index),
        Err(error) => {
            diagnostics.push(index_build_failed(&error));
            None
        }
    };
    let index = inspect_generated_index(
        &catalog_index_path(&request.user_state_dir),
        current_index.as_ref(),
        &mut diagnostics,
    );

    Ok(DoctorResult {
        catalog: DoctorCatalogSummary {
            capability_count: discovery.capabilities.len(),
            profile_count: discovery.profiles.len(),
        },
        index,
        diagnostics,
    })
}

fn unique_capability_ids(discovery: &CatalogDiscovery) -> BTreeSet<String> {
    let mut counts = BTreeMap::<String, usize>::new();
    for entry in &discovery.capabilities {
        *counts.entry(entry.manifest.id.to_string()).or_default() += 1;
    }
    counts
        .into_iter()
        .filter_map(|(id, count)| (count == 1).then_some(id))
        .collect()
}

fn validate_capability_files(discovery: &CatalogDiscovery, diagnostics: &mut Vec<Diagnostic>) {
    for entry in &discovery.capabilities {
        for (key, value) in &entry.manifest.files.entries {
            let path = entry.directory_path.join(value);
            if !path.is_file() {
                diagnostics.push(missing_capability_file(entry, key, value, &path));
            }
        }
    }
}

fn validate_capability_requirements(
    discovery: &CatalogDiscovery,
    capability_ids: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for entry in &discovery.capabilities {
        let Some(requires) = &entry.manifest.requires else {
            continue;
        };
        for required in &requires.capabilities {
            let id = required.to_string();
            if !capability_ids.contains(&id) {
                diagnostics.push(missing_required_capability(entry, &id));
            }
        }
    }
}

fn validate_profile_references(
    discovery: &CatalogDiscovery,
    capability_ids: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for entry in &discovery.profiles {
        for id in &entry.manifest.capabilities {
            let id = id.to_string();
            if !capability_ids.contains(&id) {
                diagnostics.push(missing_profile_capability(entry, "capabilities", &id));
            }
        }
        for id in &entry.manifest.instructions {
            let id = id.to_string();
            if !capability_ids.contains(&id) {
                diagnostics.push(missing_profile_capability(entry, "instructions", &id));
            }
        }
    }
}

fn inspect_generated_index(
    path: &Path,
    current: Option<&CatalogIndex>,
    diagnostics: &mut Vec<Diagnostic>,
) -> DoctorIndexSummary {
    let current_fingerprint = current.map(|index| index.content_fingerprint.clone());
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            return DoctorIndexSummary {
                path: path.to_path_buf(),
                status: DoctorIndexStatus::Missing,
                generated_fingerprint: None,
                current_fingerprint,
            };
        }
        Err(source) => {
            diagnostics.push(index_read_failed(path, &source));
            return DoctorIndexSummary {
                path: path.to_path_buf(),
                status: DoctorIndexStatus::ReadFailed,
                generated_fingerprint: None,
                current_fingerprint,
            };
        }
    };

    let generated = match serde_json::from_str::<CatalogIndex>(&raw) {
        Ok(index) => index,
        Err(source) => {
            diagnostics.push(index_corrupt(path, &source.to_string()));
            return DoctorIndexSummary {
                path: path.to_path_buf(),
                status: DoctorIndexStatus::Corrupt,
                generated_fingerprint: None,
                current_fingerprint,
            };
        }
    };
    let generated_fingerprint = Some(generated.content_fingerprint.clone());
    let status = match current {
        Some(current) if index_is_fresh(&generated, current) => DoctorIndexStatus::Fresh,
        Some(current) => {
            diagnostics.push(index_stale(path, &generated, current));
            DoctorIndexStatus::Stale
        }
        None => DoctorIndexStatus::Unchecked,
    };

    DoctorIndexSummary {
        path: path.to_path_buf(),
        status,
        generated_fingerprint,
        current_fingerprint,
    }
}

fn index_is_fresh(generated: &CatalogIndex, current: &CatalogIndex) -> bool {
    generated.schema_version == current.schema_version
        && generated.content_fingerprint == current.content_fingerprint
}

fn missing_capability_file(
    entry: &DiscoveredCapabilityManifest,
    key: &str,
    value: &str,
    path: &Path,
) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "catalog.capability-file-missing",
        format!(
            "capability `{}` references missing file `{}` at `{}`",
            entry.manifest.id,
            value,
            path.display()
        ),
    )
    .with_location(DiagnosticLocation::manifest_field(
        &entry.manifest_path,
        format!("files.{key}"),
    ))
    .with_recovery_hint("add the referenced file or update the capability manifest")
}

fn missing_required_capability(entry: &DiscoveredCapabilityManifest, id: &str) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "catalog.capability-requirement-not-found",
        format!(
            "capability `{}` requires missing capability `{id}`",
            entry.manifest.id
        ),
    )
    .with_location(DiagnosticLocation::manifest_field(
        &entry.manifest_path,
        "requires.capabilities",
    ))
    .with_recovery_hint("add the required capability or remove the requirement")
}

fn missing_profile_capability(
    entry: &DiscoveredProfileManifest,
    field: &'static str,
    id: &str,
) -> Diagnostic {
    let code = if field == "instructions" {
        "profile.instruction-not-found"
    } else {
        "profile.capability-not-found"
    };
    Diagnostic::new(
        DiagnosticSeverity::Error,
        code,
        format!(
            "profile `{}` references missing capability `{id}` in `{field}`",
            entry.manifest.id
        ),
    )
    .with_location(DiagnosticLocation::manifest_field(
        &entry.manifest_path,
        field,
    ))
    .with_recovery_hint("run `agent-matters capabilities list` to inspect exact capability ids")
}

fn index_stale(path: &Path, generated: &CatalogIndex, current: &CatalogIndex) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Warning,
        "catalog.index-stale",
        format!(
            "generated catalog index `{}` is stale: generated {}, current {}",
            path.display(),
            generated.content_fingerprint,
            current.content_fingerprint
        ),
    )
    .with_location(DiagnosticLocation::manifest_path(path))
    .with_recovery_hint("run any catalog command to rebuild the generated index")
}

fn index_corrupt(path: &Path, detail: &str) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Warning,
        "catalog.index-corrupt",
        format!(
            "generated catalog index `{}` is corrupt: {detail}",
            path.display()
        ),
    )
    .with_location(DiagnosticLocation::manifest_path(path))
    .with_recovery_hint("delete the generated index or rerun a catalog command to rebuild it")
}

fn index_read_failed(path: &Path, source: &io::Error) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Warning,
        "catalog.index-read-failed",
        format!(
            "failed to read generated catalog index `{}`: {source}",
            path.display()
        ),
    )
    .with_location(DiagnosticLocation::manifest_path(path))
}

fn index_build_failed(error: &CatalogIndexError) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "catalog.index-build-failed",
        format!("failed to build current catalog index: {error}"),
    )
}
