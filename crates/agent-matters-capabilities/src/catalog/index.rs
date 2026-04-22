//! Generated catalog index use cases.

use std::fs;
use std::io;
use std::path::Path;

use agent_matters_core::catalog::{CATALOG_INDEX_SCHEMA_VERSION, CatalogIndex};

use super::{CatalogDiscovery, discover_catalog};

mod diagnostics;
mod fingerprint;
mod paths;
mod records;
mod types;

use diagnostics::corrupt_index_diagnostic;
use fingerprint::content_fingerprint;
pub use paths::catalog_index_path;
use records::{capability_record, duplicate_capability_ids, duplicate_profile_ids, profile_record};
pub use types::{
    CatalogIndexError, CatalogIndexStatus, LoadCatalogIndexRequest, LoadCatalogIndexResult,
};

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
