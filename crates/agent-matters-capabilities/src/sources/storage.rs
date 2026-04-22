mod error;
mod fs;
mod partial;
mod paths;
mod provenance;
mod publish;
mod staging;

use std::path::PathBuf;

use agent_matters_core::catalog::{CATALOG_DIR_NAME, capability_kind_dir_name};

use super::contract::SourceImportResult;
pub use error::SourceImportStorageError;
use paths::{
    import_tree_paths, reject_complete_existing_import, temp_sibling, validated_vendor_dir,
};
use provenance::validate_provenance;
use publish::publish_staged_import;
use staging::{cleanup_staging, prepare_staging, write_staged_import};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteSourceImportRequest {
    pub repo_root: PathBuf,
    pub import: SourceImportResult,
    pub replace_existing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteSourceImportResult {
    pub capability_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub vendor_dir: PathBuf,
    pub catalog_files: Vec<PathBuf>,
    pub vendor_files: Vec<PathBuf>,
    pub diagnostics: Vec<agent_matters_core::domain::Diagnostic>,
}

pub fn write_source_import(
    request: WriteSourceImportRequest,
) -> Result<WriteSourceImportResult, SourceImportStorageError> {
    validate_provenance(&request.import)?;
    if request.import.vendor_files.is_empty() {
        return Err(SourceImportStorageError::MissingVendorRecord {
            source_id: request.import.source.clone(),
            locator: request.import.locator.clone(),
        });
    }

    let capability_dir = request
        .repo_root
        .join(CATALOG_DIR_NAME)
        .join(capability_kind_dir_name(request.import.manifest.kind))
        .join(request.import.manifest.id.body());
    let vendor_dir = validated_vendor_dir(
        &request.repo_root,
        &request.import.source,
        &request.import.locator,
    )?;

    let final_paths = import_tree_paths(
        capability_dir,
        vendor_dir,
        &request.import.catalog_files,
        &request.import.vendor_files,
    )?;

    if !request.replace_existing {
        reject_complete_existing_import(&final_paths)?;
    }

    let manifest = toml::to_string_pretty(&request.import.manifest).map_err(|source| {
        SourceImportStorageError::SerializeManifest {
            capability: request.import.manifest.id.to_string(),
            source,
        }
    })?;

    let staging_paths = import_tree_paths(
        temp_sibling(&final_paths.capability_dir, "source-import"),
        temp_sibling(&final_paths.vendor_dir, "source-import"),
        &request.import.catalog_files,
        &request.import.vendor_files,
    )?;
    prepare_staging(&staging_paths)?;

    let write_result = write_staged_import(&request.import, &manifest, &staging_paths);
    if let Err(source) = write_result {
        cleanup_staging(&staging_paths);
        return Err(source);
    }

    let publish_result =
        publish_staged_import(&staging_paths, &final_paths, request.replace_existing);
    if let Err(source) = publish_result {
        cleanup_staging(&staging_paths);
        return Err(source);
    }

    Ok(WriteSourceImportResult {
        capability_dir: final_paths.capability_dir,
        manifest_path: final_paths.manifest_path,
        vendor_dir: final_paths.vendor_dir,
        catalog_files: final_paths.catalog_files,
        vendor_files: final_paths.vendor_files,
        diagnostics: request.import.diagnostics,
    })
}
