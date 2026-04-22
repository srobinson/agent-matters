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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImportTreePaths {
    capability_dir: PathBuf,
    manifest_path: PathBuf,
    vendor_dir: PathBuf,
    catalog_files: Vec<PathBuf>,
    vendor_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReplacementBackupPaths {
    capability_dir: PathBuf,
    vendor_dir: PathBuf,
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
    #[error("failed to publish source import path `{from}` to `{to}`: {source}")]
    PublishPath {
        from: PathBuf,
        to: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to remove source import path `{path}`: {source}")]
    RemovePath {
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
        reject_existing(&final_paths.capability_dir)?;
        reject_existing(&final_paths.vendor_dir)?;
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
    remove_path_if_exists(&staging_paths.capability_dir)?;
    remove_path_if_exists(&staging_paths.vendor_dir)?;

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

fn import_tree_paths(
    capability_dir: PathBuf,
    vendor_dir: PathBuf,
    catalog_files: &[SourceImportFile],
    vendor_files: &[SourceImportFile],
) -> Result<ImportTreePaths, SourceImportStorageError> {
    let manifest_path = capability_dir.join(MANIFEST_FILE_NAME);
    let catalog_files = validate_file_set(&capability_dir, catalog_files, true)?;
    let vendor_files = validate_file_set(&vendor_dir, vendor_files, false)?;

    Ok(ImportTreePaths {
        capability_dir,
        manifest_path,
        vendor_dir,
        catalog_files,
        vendor_files,
    })
}

fn validated_vendor_dir(
    repo_root: &Path,
    source: &str,
    locator: &str,
) -> Result<PathBuf, SourceImportStorageError> {
    let source_path = Path::new(source);
    let locator_path = Path::new(locator);
    validate_relative_path(source_path)?;
    validate_relative_path(locator_path)?;

    Ok(repo_root
        .join(VENDOR_DIR_NAME)
        .join(source_path)
        .join(locator_path))
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

fn write_staged_import(
    import: &SourceImportResult,
    manifest: &str,
    paths: &ImportTreePaths,
) -> Result<(), SourceImportStorageError> {
    create_dir_all(&paths.capability_dir)?;
    create_dir_all(&paths.vendor_dir)?;
    write_file(&paths.manifest_path, manifest)?;
    write_file_set(&import.catalog_files, &paths.catalog_files)?;
    write_file_set(&import.vendor_files, &paths.vendor_files)?;
    Ok(())
}

fn publish_staged_import(
    staging_paths: &ImportTreePaths,
    final_paths: &ImportTreePaths,
    replace_existing: bool,
) -> Result<(), SourceImportStorageError> {
    if replace_existing {
        return publish_replacing_staged_import(staging_paths, final_paths);
    }

    publish_new_staged_import(staging_paths, final_paths)
}

fn publish_new_staged_import(
    staging_paths: &ImportTreePaths,
    final_paths: &ImportTreePaths,
) -> Result<(), SourceImportStorageError> {
    publish_path(&staging_paths.capability_dir, &final_paths.capability_dir)?;
    match publish_path(&staging_paths.vendor_dir, &final_paths.vendor_dir) {
        Ok(()) => Ok(()),
        Err(source) => {
            let _ = remove_path_if_exists(&final_paths.capability_dir);
            Err(source)
        }
    }
}

fn publish_replacing_staged_import(
    staging_paths: &ImportTreePaths,
    final_paths: &ImportTreePaths,
) -> Result<(), SourceImportStorageError> {
    let backup_paths = ReplacementBackupPaths {
        capability_dir: temp_sibling(&final_paths.capability_dir, "source-import-backup"),
        vendor_dir: temp_sibling(&final_paths.vendor_dir, "source-import-backup"),
    };
    remove_path_if_exists(&backup_paths.capability_dir)?;
    remove_path_if_exists(&backup_paths.vendor_dir)?;

    let capability_backed_up =
        move_path_if_exists(&final_paths.capability_dir, &backup_paths.capability_dir)?;
    let vendor_backed_up =
        match move_path_if_exists(&final_paths.vendor_dir, &backup_paths.vendor_dir) {
            Ok(backed_up) => backed_up,
            Err(source) => {
                restore_replacement_backups(
                    &backup_paths,
                    final_paths,
                    capability_backed_up,
                    false,
                );
                return Err(source);
            }
        };

    match publish_new_staged_import(staging_paths, final_paths) {
        Ok(()) => {
            cleanup_replacement_backups(&backup_paths);
            Ok(())
        }
        Err(source) => {
            rollback_published_replacement(
                &backup_paths,
                final_paths,
                capability_backed_up,
                vendor_backed_up,
            );
            Err(source)
        }
    }
}

fn rollback_published_replacement(
    backup_paths: &ReplacementBackupPaths,
    final_paths: &ImportTreePaths,
    capability_backed_up: bool,
    vendor_backed_up: bool,
) {
    let _ = remove_path_if_exists(&final_paths.capability_dir);
    let _ = remove_path_if_exists(&final_paths.vendor_dir);
    restore_replacement_backups(
        backup_paths,
        final_paths,
        capability_backed_up,
        vendor_backed_up,
    );
}

fn restore_replacement_backups(
    backup_paths: &ReplacementBackupPaths,
    final_paths: &ImportTreePaths,
    capability_backed_up: bool,
    vendor_backed_up: bool,
) {
    if capability_backed_up {
        let _ = publish_path(&backup_paths.capability_dir, &final_paths.capability_dir);
    }
    if vendor_backed_up {
        let _ = publish_path(&backup_paths.vendor_dir, &final_paths.vendor_dir);
    }
}

fn cleanup_replacement_backups(paths: &ReplacementBackupPaths) {
    let _ = remove_path_if_exists(&paths.capability_dir);
    let _ = remove_path_if_exists(&paths.vendor_dir);
}

fn cleanup_staging(paths: &ImportTreePaths) {
    let _ = remove_path_if_exists(&paths.capability_dir);
    let _ = remove_path_if_exists(&paths.vendor_dir);
}

fn validated_child_path(base: &Path, relative: &Path) -> Result<PathBuf, SourceImportStorageError> {
    validate_relative_path(relative)?;
    Ok(base.join(relative))
}

fn validate_relative_path(relative: &Path) -> Result<(), SourceImportStorageError> {
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

    Ok(())
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

fn publish_path(from: &Path, to: &Path) -> Result<(), SourceImportStorageError> {
    fs::rename(from, to).map_err(|source| SourceImportStorageError::PublishPath {
        from: from.to_path_buf(),
        to: to.to_path_buf(),
        source,
    })
}

fn move_path_if_exists(from: &Path, to: &Path) -> Result<bool, SourceImportStorageError> {
    match fs::symlink_metadata(from) {
        Ok(_) => {
            publish_path(from, to)?;
            Ok(true)
        }
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(SourceImportStorageError::PublishPath {
            from: from.to_path_buf(),
            to: to.to_path_buf(),
            source,
        }),
    }
}

fn remove_path_if_exists(path: &Path) -> Result<(), SourceImportStorageError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(SourceImportStorageError::RemovePath {
                path: path.to_path_buf(),
                source,
            });
        }
    };

    let result = if metadata.is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    result.map_err(|source| SourceImportStorageError::RemovePath {
        path: path.to_path_buf(),
        source,
    })
}

fn temp_sibling(path: &Path, label: &str) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("source-import");
    path.with_file_name(format!(".{name}.{label}.tmp-{}", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    #[test]
    fn replacing_publish_restores_existing_trees_when_vendor_publish_fails() {
        let repo = TempDir::new().unwrap();
        let final_paths = test_paths(
            repo.path().join("catalog/skills/playwright"),
            repo.path().join("vendor/skills.sh/playwright"),
        );
        let staging_paths = test_paths(
            repo.path()
                .join("catalog/skills/.playwright.source-import.tmp-test"),
            repo.path()
                .join("vendor/skills.sh/.playwright.source-import.tmp-test"),
        );
        fs::create_dir_all(&final_paths.capability_dir).unwrap();
        fs::create_dir_all(&final_paths.vendor_dir).unwrap();
        fs::create_dir_all(&staging_paths.capability_dir).unwrap();
        fs::write(final_paths.capability_dir.join("old.txt"), "old capability").unwrap();
        fs::write(final_paths.vendor_dir.join("old.txt"), "old vendor").unwrap();
        fs::write(
            staging_paths.capability_dir.join("new.txt"),
            "new capability",
        )
        .unwrap();

        let err = publish_staged_import(&staging_paths, &final_paths, true).unwrap_err();

        assert!(matches!(err, SourceImportStorageError::PublishPath { .. }));
        assert_eq!(
            fs::read_to_string(final_paths.capability_dir.join("old.txt")).unwrap(),
            "old capability"
        );
        assert_eq!(
            fs::read_to_string(final_paths.vendor_dir.join("old.txt")).unwrap(),
            "old vendor"
        );
        assert!(!final_paths.capability_dir.join("new.txt").exists());
    }

    fn test_paths(capability_dir: PathBuf, vendor_dir: PathBuf) -> ImportTreePaths {
        ImportTreePaths {
            manifest_path: capability_dir.join(MANIFEST_FILE_NAME),
            capability_dir,
            vendor_dir,
            catalog_files: Vec::new(),
            vendor_files: Vec::new(),
        }
    }
}
