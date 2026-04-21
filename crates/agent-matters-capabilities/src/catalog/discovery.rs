//! Filesystem discovery for authored catalog manifests.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use agent_matters_core::catalog::{
    CATALOG_DIR_NAME, MANIFEST_FILE_NAME, PROFILES_DIR_NAME, capability_kind_dir_name,
    known_capability_dir_names,
};
use agent_matters_core::domain::{
    CapabilityKind, Diagnostic, DiagnosticLocation, DiagnosticSeverity,
};
use agent_matters_core::manifest::{CapabilityManifest, ProfileManifest};
use serde::de::DeserializeOwned;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CatalogDiscovery {
    pub capabilities: Vec<DiscoveredCapabilityManifest>,
    pub profiles: Vec<DiscoveredProfileManifest>,
    pub diagnostics: Vec<Diagnostic>,
}

pub type DiscoveredCapabilityManifest = DiscoveredManifest<CapabilityManifest>;
pub type DiscoveredProfileManifest = DiscoveredManifest<ProfileManifest>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredManifest<T> {
    pub manifest: T,
    pub manifest_path: PathBuf,
    pub directory_path: PathBuf,
}

pub fn discover_catalog(repo_root: &Path) -> CatalogDiscovery {
    let mut discovery = CatalogDiscovery::default();

    report_unknown_catalog_dirs(repo_root, &mut discovery.diagnostics);
    discover_capabilities(repo_root, &mut discovery);
    discover_profiles(repo_root, &mut discovery);
    report_duplicate_ids(&mut discovery);

    discovery
}

fn discover_capabilities(repo_root: &Path, discovery: &mut CatalogDiscovery) {
    for kind in CapabilityKind::all() {
        let root = repo_root
            .join(CATALOG_DIR_NAME)
            .join(capability_kind_dir_name(*kind));

        let Some(entries) = read_dir_if_present(&root, &mut discovery.diagnostics) else {
            continue;
        };

        for path in entries {
            if !path.is_dir() {
                continue;
            }
            let manifest_path = path.join(MANIFEST_FILE_NAME);
            match load_manifest::<CapabilityManifest>(&manifest_path, &mut discovery.diagnostics) {
                Some(manifest) => {
                    report_capability_kind_mismatches(
                        *kind,
                        &manifest,
                        &manifest_path,
                        &mut discovery.diagnostics,
                    );
                    discovery.capabilities.push(DiscoveredManifest {
                        manifest,
                        manifest_path,
                        directory_path: path,
                    });
                }
                None if !manifest_path.exists() => report_missing_manifest(
                    &manifest_path,
                    "capability directory is missing manifest.toml",
                    &mut discovery.diagnostics,
                ),
                None => {}
            }
        }
    }
}

fn discover_profiles(repo_root: &Path, discovery: &mut CatalogDiscovery) {
    let root = repo_root.join(PROFILES_DIR_NAME);
    let Some(entries) = read_dir_if_present(&root, &mut discovery.diagnostics) else {
        return;
    };

    for path in entries {
        if !path.is_dir() {
            continue;
        }
        let manifest_path = path.join(MANIFEST_FILE_NAME);
        match load_manifest::<ProfileManifest>(&manifest_path, &mut discovery.diagnostics) {
            Some(manifest) => discovery.profiles.push(DiscoveredManifest {
                manifest,
                manifest_path,
                directory_path: path,
            }),
            None if !manifest_path.exists() => report_missing_manifest(
                &manifest_path,
                "profile directory is missing manifest.toml",
                &mut discovery.diagnostics,
            ),
            None => {}
        }
    }
}

fn report_unknown_catalog_dirs(repo_root: &Path, diagnostics: &mut Vec<Diagnostic>) {
    let root = repo_root.join(CATALOG_DIR_NAME);
    let Some(entries) = read_dir_if_present(&root, diagnostics) else {
        return;
    };

    for path in entries {
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if known_capability_dir_names().contains(&name) {
            continue;
        }
        diagnostics.push(
            Diagnostic::new(
                DiagnosticSeverity::Warning,
                "catalog.unknown-folder",
                format!("unknown catalog folder `{}`", path.display()),
            )
            .with_location(DiagnosticLocation::manifest_path(path))
            .with_recovery_hint(format!(
                "use one of: {}",
                known_capability_dir_names().join(", ")
            )),
        );
    }
}

fn load_manifest<T>(path: &Path, diagnostics: &mut Vec<Diagnostic>) -> Option<T>
where
    T: DeserializeOwned,
{
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return None,
        Err(source) => {
            diagnostics.push(
                Diagnostic::new(
                    DiagnosticSeverity::Error,
                    "catalog.manifest-read-failed",
                    format!("failed to read manifest `{}`: {source}", path.display()),
                )
                .with_location(DiagnosticLocation::manifest_path(path)),
            );
            return None;
        }
    };

    match toml::from_str::<T>(&raw) {
        Ok(manifest) => Some(manifest),
        Err(source) => {
            diagnostics.push(
                Diagnostic::new(
                    DiagnosticSeverity::Error,
                    "catalog.manifest-invalid",
                    format!("failed to parse manifest `{}`: {source}", path.display()),
                )
                .with_location(DiagnosticLocation::manifest_path(path)),
            );
            None
        }
    }
}

fn read_dir_if_present(root: &Path, diagnostics: &mut Vec<Diagnostic>) -> Option<Vec<PathBuf>> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return None,
        Err(source) => {
            diagnostics.push(
                Diagnostic::new(
                    DiagnosticSeverity::Error,
                    "catalog.directory-read-failed",
                    format!(
                        "failed to read catalog directory `{}`: {source}",
                        root.display()
                    ),
                )
                .with_location(DiagnosticLocation::manifest_path(root)),
            );
            return None;
        }
    };

    let mut paths = Vec::new();
    for entry in entries {
        match entry {
            Ok(entry) => paths.push(entry.path()),
            Err(source) => diagnostics.push(Diagnostic::new(
                DiagnosticSeverity::Error,
                "catalog.directory-entry-read-failed",
                format!(
                    "failed to read directory entry under `{}`: {source}",
                    root.display()
                ),
            )),
        }
    }
    paths.sort();
    Some(paths)
}

fn report_missing_manifest(path: &Path, message: &str, diagnostics: &mut Vec<Diagnostic>) {
    diagnostics.push(
        Diagnostic::new(
            DiagnosticSeverity::Error,
            "catalog.manifest-missing",
            message,
        )
        .with_location(DiagnosticLocation::manifest_path(path))
        .with_recovery_hint("add a manifest.toml file to this directory"),
    );
}

fn report_capability_kind_mismatches(
    expected_kind: CapabilityKind,
    manifest: &CapabilityManifest,
    manifest_path: &Path,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if manifest.id.kind() != manifest.kind {
        diagnostics.push(
            Diagnostic::new(
                DiagnosticSeverity::Error,
                "catalog.manifest-kind-mismatch",
                format!(
                    "capability id kind `{}` does not match manifest kind `{}`",
                    manifest.id.kind(),
                    manifest.kind
                ),
            )
            .with_location(DiagnosticLocation::manifest_field(manifest_path, "id"))
            .with_recovery_hint("make the capability id prefix and kind field match"),
        );
    }

    if manifest.kind != expected_kind {
        diagnostics.push(
            Diagnostic::new(
                DiagnosticSeverity::Error,
                "catalog.directory-kind-mismatch",
                format!(
                    "capability kind `{}` belongs under `catalog/{}`, not `catalog/{}`",
                    manifest.kind,
                    capability_kind_dir_name(manifest.kind),
                    capability_kind_dir_name(expected_kind)
                ),
            )
            .with_location(DiagnosticLocation::manifest_field(manifest_path, "kind"))
            .with_recovery_hint(
                "move the capability directory under the matching catalog kind folder or update the manifest kind",
            ),
        );
    }
}

fn report_duplicate_ids(discovery: &mut CatalogDiscovery) {
    let mut capabilities = BTreeMap::<String, PathBuf>::new();
    for entry in &discovery.capabilities {
        let id = entry.manifest.id.to_string();
        if let Some(first_path) = capabilities.get(&id) {
            discovery.diagnostics.push(duplicate_id_diagnostic(
                "capability",
                &id,
                first_path,
                &entry.manifest_path,
            ));
        } else {
            capabilities.insert(id, entry.manifest_path.clone());
        }
    }

    let mut profiles = BTreeMap::<String, PathBuf>::new();
    for entry in &discovery.profiles {
        let id = entry.manifest.id.to_string();
        if let Some(first_path) = profiles.get(&id) {
            discovery.diagnostics.push(duplicate_id_diagnostic(
                "profile",
                &id,
                first_path,
                &entry.manifest_path,
            ));
        } else {
            profiles.insert(id, entry.manifest_path.clone());
        }
    }
}

fn duplicate_id_diagnostic(
    manifest_kind: &str,
    id: &str,
    first_path: &Path,
    duplicate_path: &Path,
) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "catalog.duplicate-id",
        format!(
            "duplicate {manifest_kind} id `{id}` also declared in `{}`",
            first_path.display()
        ),
    )
    .with_location(DiagnosticLocation::manifest_field(duplicate_path, "id"))
    .with_recovery_hint("make each manifest id unique")
}
