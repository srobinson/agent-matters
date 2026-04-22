use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use agent_matters_core::catalog::PROFILES_DIR_NAME;
use agent_matters_core::domain::{Diagnostic, DiagnosticLocation, DiagnosticSeverity};

use crate::catalog::CatalogDiscovery;

use super::{PlannedCapability, RuntimeHomeImportError, RuntimeImportPlan};

pub(super) fn detect_conflicts(
    repo_root: &Path,
    plan: &RuntimeImportPlan,
    discovery: &CatalogDiscovery,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let existing_capabilities = discovery
        .capabilities
        .iter()
        .map(|entry| entry.manifest.id.to_string())
        .collect::<BTreeSet<_>>();
    let existing_profiles = discovery
        .profiles
        .iter()
        .map(|entry| entry.manifest.id.to_string())
        .collect::<BTreeSet<_>>();
    let mut planned_ids = BTreeSet::new();

    if existing_profiles.contains(&plan.profile_id) {
        diagnostics.push(conflict(
            "source.runtime-import-profile-conflict",
            format!("profile `{}` already exists", plan.profile_id),
            &plan.profile_manifest_path,
        ));
    }
    if repo_root.join(&plan.profile_manifest_path).exists() {
        diagnostics.push(conflict(
            "source.runtime-import-path-conflict",
            format!(
                "runtime import target `{}` already exists",
                plan.profile_manifest_path.display()
            ),
            &plan.profile_manifest_path,
        ));
    }

    for capability in &plan.capabilities {
        let id = capability.manifest.id.to_string();
        if !planned_ids.insert(id.clone()) {
            diagnostics.push(conflict(
                "source.runtime-import-duplicate-planned-capability",
                format!("runtime import would create duplicate capability `{id}`"),
                &capability.manifest_path,
            ));
        }
        if existing_capabilities.contains(&id) {
            diagnostics.push(conflict(
                "source.runtime-import-capability-conflict",
                format!("capability `{id}` already exists"),
                &capability.manifest_path,
            ));
        }
        if repo_root.join(&capability.manifest_path).exists() {
            diagnostics.push(conflict(
                "source.runtime-import-path-conflict",
                format!(
                    "runtime import target `{}` already exists",
                    capability.manifest_path.display()
                ),
                &capability.manifest_path,
            ));
        }
        if repo_root.join(&capability.vendor_path).exists() {
            diagnostics.push(conflict(
                "source.runtime-import-vendor-conflict",
                format!(
                    "runtime import vendor target `{}` already exists",
                    capability.vendor_path.display()
                ),
                &capability.vendor_path,
            ));
        }
    }
    diagnostics
}

pub(super) fn write_runtime_import_plan(
    repo_root: &Path,
    plan: &RuntimeImportPlan,
) -> Result<(), RuntimeHomeImportError> {
    let profile_dir = repo_root.join(PROFILES_DIR_NAME).join(&plan.profile_id);
    fs::create_dir_all(&profile_dir).map_err(|source| RuntimeHomeImportError::Write {
        path: profile_dir.clone(),
        source,
    })?;
    write_toml_manifest(
        repo_root.join(&plan.profile_manifest_path),
        &plan.profile_id,
        &plan.profile,
    )?;
    for capability in &plan.capabilities {
        write_capability(repo_root, capability)?;
    }
    Ok(())
}

fn write_capability(
    repo_root: &Path,
    capability: &PlannedCapability,
) -> Result<(), RuntimeHomeImportError> {
    let manifest_path = repo_root.join(&capability.manifest_path);
    let capability_dir = manifest_path
        .parent()
        .expect("capability manifest has parent")
        .to_path_buf();
    fs::create_dir_all(&capability_dir).map_err(|source| RuntimeHomeImportError::Write {
        path: capability_dir.clone(),
        source,
    })?;
    write_toml_manifest(
        manifest_path,
        &capability.manifest.id.to_string(),
        &capability.manifest,
    )?;

    for file in &capability.catalog_files {
        let path = capability_dir.join(&file.relative_path);
        write_bytes(path, &file.contents)?;
    }

    let vendor_dir = repo_root.join(&capability.vendor_path);
    fs::create_dir_all(&vendor_dir).map_err(|source| RuntimeHomeImportError::Write {
        path: vendor_dir.clone(),
        source,
    })?;
    let record = serde_json::to_vec_pretty(&capability.vendor_record).map_err(|source| {
        RuntimeHomeImportError::SerializeVendorRecord {
            id: capability.manifest.id.to_string(),
            source,
        }
    })?;
    write_bytes(vendor_dir.join("record.json"), &record)?;
    Ok(())
}

fn write_toml_manifest<T: serde::Serialize>(
    path: PathBuf,
    id: &str,
    manifest: &T,
) -> Result<(), RuntimeHomeImportError> {
    let encoded = toml::to_string_pretty(manifest).map_err(|source| {
        RuntimeHomeImportError::SerializeManifest {
            id: id.to_string(),
            source,
        }
    })?;
    write_bytes(path, encoded.as_bytes())
}

fn write_bytes(path: PathBuf, contents: &[u8]) -> Result<(), RuntimeHomeImportError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| RuntimeHomeImportError::Write {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(&path, contents).map_err(|source| RuntimeHomeImportError::Write { path, source })
}

fn conflict(code: &'static str, message: String, path: &Path) -> Diagnostic {
    Diagnostic::new(DiagnosticSeverity::Error, code, message)
        .with_location(DiagnosticLocation::manifest_path(path))
        .with_recovery_hint("choose a different profile id or resolve the existing catalog entry")
}
