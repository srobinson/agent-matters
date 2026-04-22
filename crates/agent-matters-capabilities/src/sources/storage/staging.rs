use std::path::PathBuf;

use super::super::contract::{SourceImportFile, SourceImportResult};
use super::SourceImportStorageError;
use super::fs::{create_dir_all, remove_path_if_exists, write_file};
use super::paths::ImportTreePaths;

pub(super) fn prepare_staging(paths: &ImportTreePaths) -> Result<(), SourceImportStorageError> {
    remove_path_if_exists(&paths.capability_dir)?;
    remove_path_if_exists(&paths.vendor_dir)
}

pub(super) fn cleanup_staging(paths: &ImportTreePaths) {
    let _ = remove_path_if_exists(&paths.capability_dir);
    let _ = remove_path_if_exists(&paths.vendor_dir);
}

pub(super) fn write_staged_import(
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
