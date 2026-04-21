//! Generated catalog index use cases.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use agent_matters_core::catalog::{
    CATALOG_INDEX_FILE_NAME, CATALOG_INDEX_SCHEMA_VERSION, CapabilityIndexRecord,
    CapabilitySourceSummary, CatalogIndex, INDEXES_DIR_NAME, ProfileIndexRecord, ProvenanceSummary,
    RequirementSummary, RuntimeCompatibilitySummary,
};
use agent_matters_core::domain::{Diagnostic, DiagnosticLocation, DiagnosticSeverity, Provenance};
use serde::Serialize;

use super::{
    CapabilityDiscoverySource, CatalogDiscovery, DiscoveredCapabilityManifest,
    DiscoveredProfileManifest, discover_catalog,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadCatalogIndexRequest {
    pub repo_root: PathBuf,
    pub user_state_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadCatalogIndexResult {
    pub index: CatalogIndex,
    pub index_path: PathBuf,
    pub status: CatalogIndexStatus,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CatalogIndexStatus {
    Fresh,
    RebuiltMissing,
    RebuiltStale,
    RecoveredCorrupt,
}

#[derive(Debug, thiserror::Error)]
pub enum CatalogIndexError {
    #[error("failed to read generated index `{path}`: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to write generated index `{path}`: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to encode generated index `{path}`: {source}")]
    Encode {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

pub fn catalog_index_path(user_state_dir: &Path) -> PathBuf {
    user_state_dir
        .join(INDEXES_DIR_NAME)
        .join(CATALOG_INDEX_FILE_NAME)
}

pub fn load_or_refresh_catalog_index(
    request: LoadCatalogIndexRequest,
) -> Result<LoadCatalogIndexResult, CatalogIndexError> {
    let mut discovery = discover_catalog(&request.repo_root);
    let rebuilt = build_catalog_index(&request.repo_root, &discovery)?;
    let index_path = catalog_index_path(&request.user_state_dir);

    let existing = match fs::read_to_string(&index_path) {
        Ok(raw) => Some(raw),
        Err(source) if source.kind() == io::ErrorKind::NotFound => None,
        Err(source) => {
            return Err(CatalogIndexError::Read {
                path: index_path,
                source,
            });
        }
    };

    match existing {
        None => {
            write_index(&index_path, &rebuilt)?;
            Ok(LoadCatalogIndexResult {
                index: rebuilt,
                index_path,
                status: CatalogIndexStatus::RebuiltMissing,
                diagnostics: discovery.diagnostics,
            })
        }
        Some(raw) => match serde_json::from_str::<CatalogIndex>(&raw) {
            Ok(index) if index_is_fresh(&index, &rebuilt) => Ok(LoadCatalogIndexResult {
                index,
                index_path,
                status: CatalogIndexStatus::Fresh,
                diagnostics: discovery.diagnostics,
            }),
            Ok(_) => {
                write_index(&index_path, &rebuilt)?;
                Ok(LoadCatalogIndexResult {
                    index: rebuilt,
                    index_path,
                    status: CatalogIndexStatus::RebuiltStale,
                    diagnostics: discovery.diagnostics,
                })
            }
            Err(source) => {
                discovery
                    .diagnostics
                    .push(corrupt_index_diagnostic(&index_path, &source.to_string()));
                write_index(&index_path, &rebuilt)?;
                Ok(LoadCatalogIndexResult {
                    index: rebuilt,
                    index_path,
                    status: CatalogIndexStatus::RecoveredCorrupt,
                    diagnostics: discovery.diagnostics,
                })
            }
        },
    }
}

pub fn build_catalog_index(
    repo_root: &Path,
    discovery: &CatalogDiscovery,
) -> Result<CatalogIndex, CatalogIndexError> {
    let capability_duplicates = duplicate_capability_ids(discovery);
    let profile_duplicates = duplicate_profile_ids(discovery);

    let capabilities = discovery
        .capabilities
        .iter()
        .filter(|entry| !capability_duplicates.contains(&entry.manifest.id.to_string()))
        .map(|entry| {
            (
                entry.manifest.id.to_string(),
                capability_record(repo_root, entry),
            )
        })
        .collect();

    let profiles = discovery
        .profiles
        .iter()
        .filter(|entry| !profile_duplicates.contains(&entry.manifest.id.to_string()))
        .map(|entry| {
            (
                entry.manifest.id.to_string(),
                profile_record(repo_root, entry),
            )
        })
        .collect();

    Ok(CatalogIndex::new(
        content_fingerprint(repo_root, discovery)?,
        capabilities,
        profiles,
    ))
}

fn index_is_fresh(existing: &CatalogIndex, rebuilt: &CatalogIndex) -> bool {
    existing.schema_version == CATALOG_INDEX_SCHEMA_VERSION
        && existing.content_fingerprint == rebuilt.content_fingerprint
}

fn write_index(path: &Path, index: &CatalogIndex) -> Result<(), CatalogIndexError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CatalogIndexError::Write {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let mut encoded =
        serde_json::to_string_pretty(index).map_err(|source| CatalogIndexError::Encode {
            path: path.to_path_buf(),
            source,
        })?;
    encoded.push('\n');
    fs::write(path, encoded).map_err(|source| CatalogIndexError::Write {
        path: path.to_path_buf(),
        source,
    })
}

fn capability_record(
    repo_root: &Path,
    entry: &DiscoveredCapabilityManifest,
) -> CapabilityIndexRecord {
    let manifest = &entry.manifest;
    let requirements =
        manifest
            .requires
            .as_ref()
            .map_or_else(RequirementSummary::default, |requires| RequirementSummary {
                capabilities: requires
                    .capabilities
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                env: requires
                    .env
                    .iter()
                    .map(|env| env.name().to_string())
                    .collect(),
            });

    CapabilityIndexRecord {
        id: manifest.id.to_string(),
        kind: manifest.kind.as_str().to_string(),
        summary: manifest.summary.clone(),
        files: manifest.files.entries.clone(),
        source_path: relative_path(repo_root, &entry.directory_path),
        source: capability_source_summary(repo_root, entry),
        runtimes: manifest
            .runtimes
            .entries
            .iter()
            .map(|(runtime, compatibility)| {
                (
                    runtime.to_string(),
                    RuntimeCompatibilitySummary {
                        supported: compatibility.supported,
                        model: None,
                    },
                )
            })
            .collect(),
        provenance: provenance_summary(manifest.origin.as_ref()),
        requirements,
    }
}

fn capability_source_summary(
    repo_root: &Path,
    entry: &DiscoveredCapabilityManifest,
) -> CapabilitySourceSummary {
    match &entry.source {
        CapabilityDiscoverySource::Local => CapabilitySourceSummary {
            kind: "local".to_string(),
            normalized_path: Some(relative_path(repo_root, &entry.directory_path)),
            overlay_path: None,
            vendor_path: None,
        },
        CapabilityDiscoverySource::Imported { vendor_path } => CapabilitySourceSummary {
            kind: "imported".to_string(),
            normalized_path: Some(relative_path(repo_root, &entry.directory_path)),
            overlay_path: None,
            vendor_path: vendor_path
                .as_ref()
                .map(|path| relative_path(repo_root, path)),
        },
        CapabilityDiscoverySource::Overlay {
            target_directory_path,
            vendor_path,
            ..
        } => CapabilitySourceSummary {
            kind: "overlaid".to_string(),
            normalized_path: Some(relative_path(repo_root, target_directory_path)),
            overlay_path: Some(relative_path(repo_root, &entry.directory_path)),
            vendor_path: vendor_path
                .as_ref()
                .map(|path| relative_path(repo_root, path)),
        },
    }
}

fn profile_record(repo_root: &Path, entry: &DiscoveredProfileManifest) -> ProfileIndexRecord {
    let manifest = &entry.manifest;
    let runtimes = manifest
        .runtimes
        .as_ref()
        .map_or_else(BTreeMap::new, |runtimes| {
            runtimes
                .entries
                .iter()
                .map(|(runtime, compatibility)| {
                    (
                        runtime.to_string(),
                        RuntimeCompatibilitySummary {
                            supported: compatibility.enabled,
                            model: compatibility.model.clone(),
                        },
                    )
                })
                .collect()
        });
    let default_runtime = manifest
        .runtimes
        .as_ref()
        .and_then(|runtimes| runtimes.default.as_ref())
        .map(ToString::to_string);
    let instruction_markers = manifest
        .instructions_output
        .as_ref()
        .and_then(|output| output.markers);

    ProfileIndexRecord {
        id: manifest.id.to_string(),
        kind: manifest.kind.as_str().to_string(),
        summary: manifest.summary.clone(),
        capabilities: manifest
            .capabilities
            .iter()
            .map(ToString::to_string)
            .collect(),
        instructions: manifest
            .instructions
            .iter()
            .map(ToString::to_string)
            .collect(),
        scope: manifest.scope.clone().unwrap_or_default(),
        source_path: relative_path(repo_root, &entry.directory_path),
        runtimes,
        default_runtime,
        instruction_markers,
        capability_count: manifest.capabilities.len(),
        instruction_count: manifest.instructions.len(),
    }
}

fn provenance_summary(origin: Option<&Provenance>) -> ProvenanceSummary {
    match origin.unwrap_or(&Provenance::Local) {
        Provenance::Local => ProvenanceSummary {
            kind: "local".to_string(),
            source: None,
            locator: None,
            version: None,
        },
        Provenance::External {
            source,
            locator,
            version,
        } => ProvenanceSummary {
            kind: "external".to_string(),
            source: Some(source.clone()),
            locator: Some(locator.clone()),
            version: version.clone(),
        },
        Provenance::Derived {
            source,
            locator,
            version,
        } => ProvenanceSummary {
            kind: "derived".to_string(),
            source: Some(source.clone()),
            locator: Some(locator.clone()),
            version: version.clone(),
        },
        Provenance::Generated {
            source,
            locator,
            version,
        } => ProvenanceSummary {
            kind: "generated".to_string(),
            source: Some(source.clone()),
            locator: locator.clone(),
            version: version.clone(),
        },
    }
}

fn duplicate_capability_ids(discovery: &CatalogDiscovery) -> BTreeSet<String> {
    duplicate_ids(
        discovery
            .capabilities
            .iter()
            .map(|entry| entry.manifest.id.to_string()),
    )
}

fn duplicate_profile_ids(discovery: &CatalogDiscovery) -> BTreeSet<String> {
    duplicate_ids(
        discovery
            .profiles
            .iter()
            .map(|entry| entry.manifest.id.to_string()),
    )
}

fn duplicate_ids(ids: impl Iterator<Item = String>) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    let mut duplicates = BTreeSet::new();
    for id in ids {
        if !seen.insert(id.clone()) {
            duplicates.insert(id);
        }
    }
    duplicates
}

fn content_fingerprint(
    repo_root: &Path,
    discovery: &CatalogDiscovery,
) -> Result<String, CatalogIndexError> {
    let mut paths = BTreeSet::<PathBuf>::new();
    for entry in &discovery.capabilities {
        paths.insert(entry.manifest_path.clone());
        if let CapabilityDiscoverySource::Overlay {
            target_manifest_path,
            ..
        } = &entry.source
        {
            paths.insert(target_manifest_path.clone());
        }
    }
    for entry in &discovery.profiles {
        paths.insert(entry.manifest_path.clone());
    }

    let mut hasher = Fnv64::new();
    hasher.write_u16(CATALOG_INDEX_SCHEMA_VERSION);
    for path in paths {
        hasher.write(relative_path(repo_root, &path).as_bytes());
        hasher.write(&[0]);
        let bytes = fs::read(&path).map_err(|source| CatalogIndexError::Read {
            path: path.clone(),
            source,
        })?;
        hasher.write(&bytes);
        hasher.write(&[0xff]);
    }

    Ok(format!("fnv64:{:016x}", hasher.finish()))
}

fn relative_path(repo_root: &Path, path: &Path) -> String {
    path.strip_prefix(repo_root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

fn corrupt_index_diagnostic(path: &Path, detail: &str) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "catalog.index-corrupt",
        format!(
            "generated catalog index `{}` is corrupt: {detail}",
            path.display()
        ),
    )
    .with_location(DiagnosticLocation::manifest_path(path))
    .with_recovery_hint("delete the generated index or rerun the command to rebuild it")
}

struct Fnv64(u64);

impl Fnv64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    const fn new() -> Self {
        Self(Self::OFFSET)
    }

    fn write_u16(&mut self, value: u16) {
        self.write(&value.to_le_bytes());
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
    }

    const fn finish(&self) -> u64 {
        self.0
    }
}
