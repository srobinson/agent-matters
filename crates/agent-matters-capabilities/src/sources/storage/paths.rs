use std::path::{Component, Path, PathBuf};

use agent_matters_core::catalog::{MANIFEST_FILE_NAME, VENDOR_DIR_NAME};

use super::super::contract::SourceImportFile;
use super::SourceImportStorageError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ImportTreePaths {
    pub(super) capability_dir: PathBuf,
    pub(super) manifest_path: PathBuf,
    pub(super) vendor_dir: PathBuf,
    pub(super) catalog_files: Vec<PathBuf>,
    pub(super) vendor_files: Vec<PathBuf>,
}

pub(super) fn import_tree_paths(
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

pub(super) fn validated_vendor_dir(
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

pub(super) fn temp_sibling(path: &Path, label: &str) -> PathBuf {
    sibling_with_name(path, format_args!("{label}.tmp-{}", std::process::id()))
}

pub(super) fn durable_sibling(path: &Path, label: &str) -> PathBuf {
    sibling_with_name(path, format_args!("{label}"))
}

fn sibling_with_name(path: &Path, suffix: std::fmt::Arguments<'_>) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("source-import");
    path.with_file_name(format!(".{name}.{suffix}"))
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
