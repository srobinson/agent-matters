use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use agent_matters_core::catalog::{
    CATALOG_DIR_NAME, MANIFEST_FILE_NAME, VENDOR_DIR_NAME, capability_kind_dir_name,
};
use agent_matters_core::domain::Provenance;
use thiserror::Error;

use super::contract::{SourceImportFile, SourceImportResult};

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

#[derive(Debug, Error)]
pub enum SourceImportStorageError {
    #[error("import manifest for `{capability}` must include external or derived provenance")]
    MissingProvenance { capability: String },
    #[error(
        "import provenance `{origin_source}:{origin_locator}` does not match source result `{result_source}:{locator}`"
    )]
    ProvenanceMismatch {
        origin_source: String,
        origin_locator: String,
        result_source: String,
        locator: String,
    },
    #[error("import for `{source_id}:{locator}` must include at least one raw vendor file")]
    MissingVendorRecord { source_id: String, locator: String },
    #[error("refusing to replace existing import path `{path}`")]
    AlreadyExists { path: PathBuf },
    #[error("relative import path `{path}` must stay inside its target directory")]
    InvalidRelativePath { path: PathBuf },
    #[error("source import file path `{path}` is reserved for generated metadata")]
    ReservedPath { path: PathBuf },
    #[error("failed to serialize manifest for `{capability}`: {source}")]
    SerializeManifest {
        capability: String,
        #[source]
        source: toml::ser::Error,
    },
    #[error("failed to create directory `{path}`: {source}")]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to write file `{path}`: {source}")]
    WriteFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
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
    let vendor_dir = request
        .repo_root
        .join(VENDOR_DIR_NAME)
        .join(&request.import.source)
        .join(&request.import.locator);

    if !request.replace_existing {
        reject_existing(&capability_dir)?;
        reject_existing(&vendor_dir)?;
    }

    let manifest_path = capability_dir.join(MANIFEST_FILE_NAME);
    let catalog_files = validate_file_set(&capability_dir, &request.import.catalog_files, true)?;
    let vendor_files = validate_file_set(&vendor_dir, &request.import.vendor_files, false)?;
    let manifest = toml::to_string_pretty(&request.import.manifest).map_err(|source| {
        SourceImportStorageError::SerializeManifest {
            capability: request.import.manifest.id.to_string(),
            source,
        }
    })?;

    create_dir_all(&capability_dir)?;
    create_dir_all(&vendor_dir)?;
    write_file(&manifest_path, &manifest)?;
    write_file_set(&request.import.catalog_files, &catalog_files)?;
    write_file_set(&request.import.vendor_files, &vendor_files)?;

    Ok(WriteSourceImportResult {
        capability_dir,
        manifest_path,
        vendor_dir,
        catalog_files,
        vendor_files,
        diagnostics: request.import.diagnostics,
    })
}

fn validate_provenance(import: &SourceImportResult) -> Result<(), SourceImportStorageError> {
    let Some(origin) = import.manifest.origin.as_ref() else {
        return Err(SourceImportStorageError::MissingProvenance {
            capability: import.manifest.id.to_string(),
        });
    };
    let Some((origin_source, origin_locator)) = provenance_source_locator(origin) else {
        return Err(SourceImportStorageError::MissingProvenance {
            capability: import.manifest.id.to_string(),
        });
    };
    if origin_source != import.source || origin_locator != import.locator {
        return Err(SourceImportStorageError::ProvenanceMismatch {
            origin_source: origin_source.to_string(),
            origin_locator: origin_locator.to_string(),
            result_source: import.source.clone(),
            locator: import.locator.clone(),
        });
    }
    Ok(())
}

fn provenance_source_locator(origin: &Provenance) -> Option<(&str, &str)> {
    match origin {
        Provenance::External {
            source, locator, ..
        }
        | Provenance::Derived {
            source, locator, ..
        } => Some((source.as_str(), locator.as_str())),
        _ => None,
    }
}

fn reject_existing(path: &Path) -> Result<(), SourceImportStorageError> {
    if path.exists() {
        return Err(SourceImportStorageError::AlreadyExists {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

fn validate_file_set(
    base: &Path,
    files: &[SourceImportFile],
    reject_manifest: bool,
) -> Result<Vec<PathBuf>, SourceImportStorageError> {
    files
        .iter()
        .map(|file| {
            let path = validated_child_path(base, &file.relative_path)?;
            if reject_manifest && file.relative_path == Path::new(MANIFEST_FILE_NAME) {
                return Err(SourceImportStorageError::ReservedPath {
                    path: file.relative_path.clone(),
                });
            }
            Ok(path)
        })
        .collect()
}

fn write_file_set(
    files: &[SourceImportFile],
    paths: &[PathBuf],
) -> Result<(), SourceImportStorageError> {
    for (file, path) in files.iter().zip(paths) {
        if let Some(parent) = path.parent() {
            create_dir_all(parent)?;
        }
        write_file(path, &file.contents)?;
    }
    Ok(())
}

fn validated_child_path(base: &Path, relative: &Path) -> Result<PathBuf, SourceImportStorageError> {
    if relative.as_os_str().is_empty() || relative.is_absolute() {
        return Err(SourceImportStorageError::InvalidRelativePath {
            path: relative.to_path_buf(),
        });
    }

    for component in relative.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(SourceImportStorageError::InvalidRelativePath {
                path: relative.to_path_buf(),
            });
        }
    }

    Ok(base.join(relative))
}

fn create_dir_all(path: &Path) -> Result<(), SourceImportStorageError> {
    fs::create_dir_all(path).map_err(|source| SourceImportStorageError::CreateDirectory {
        path: path.to_path_buf(),
        source,
    })
}

fn write_file(path: &Path, contents: &str) -> Result<(), SourceImportStorageError> {
    fs::write(path, contents).map_err(|source| SourceImportStorageError::WriteFile {
        path: path.to_path_buf(),
        source,
    })
}
