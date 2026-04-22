use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use super::SourceImportStorageError;
use super::fs::{publish_path, remove_path_if_exists};
use super::paths::ImportTreePaths;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PublishedTree {
    Capability,
    Vendor,
}

pub(super) fn complete_partial_new_import(
    published: PublishedTree,
    staging_paths: &ImportTreePaths,
    final_paths: &ImportTreePaths,
) -> Result<(), SourceImportStorageError> {
    let paths = PartialImportPaths::new(published, staging_paths, final_paths);
    if !trees_match(paths.staged_published, paths.final_published)? {
        return Err(SourceImportStorageError::PartialPublishConflict {
            existing: paths.final_published.to_path_buf(),
            missing: paths.final_missing.to_path_buf(),
        });
    }

    remove_path_if_exists(paths.staged_published)?;
    publish_path(paths.staged_missing, paths.final_missing)
}

struct PartialImportPaths<'a> {
    staged_published: &'a Path,
    final_published: &'a Path,
    staged_missing: &'a Path,
    final_missing: &'a Path,
}

impl<'a> PartialImportPaths<'a> {
    fn new(
        published: PublishedTree,
        staging_paths: &'a ImportTreePaths,
        final_paths: &'a ImportTreePaths,
    ) -> Self {
        match published {
            PublishedTree::Capability => Self {
                staged_published: &staging_paths.capability_dir,
                final_published: &final_paths.capability_dir,
                staged_missing: &staging_paths.vendor_dir,
                final_missing: &final_paths.vendor_dir,
            },
            PublishedTree::Vendor => Self {
                staged_published: &staging_paths.vendor_dir,
                final_published: &final_paths.vendor_dir,
                staged_missing: &staging_paths.capability_dir,
                final_missing: &final_paths.capability_dir,
            },
        }
    }
}

fn trees_match(left: &Path, right: &Path) -> Result<bool, SourceImportStorageError> {
    let left_metadata = inspect_metadata(left)?;
    let right_metadata = inspect_metadata(right)?;
    if left_metadata.file_type().is_symlink() || right_metadata.file_type().is_symlink() {
        return Ok(false);
    }

    if left_metadata.is_file() && right_metadata.is_file() {
        return Ok(read_file(left)? == read_file(right)?);
    }

    if left_metadata.is_dir() && right_metadata.is_dir() {
        return directories_match(left, right);
    }

    Ok(false)
}

fn directories_match(left: &Path, right: &Path) -> Result<bool, SourceImportStorageError> {
    let left_entries = directory_entries(left)?;
    let right_entries = directory_entries(right)?;
    if left_entries.len() != right_entries.len() || !left_entries.keys().eq(right_entries.keys()) {
        return Ok(false);
    }

    for (name, left_child) in left_entries {
        let right_child = right_entries
            .get(&name)
            .expect("matched directory entries should share keys");
        if !trees_match(&left_child, right_child)? {
            return Ok(false);
        }
    }

    Ok(true)
}

fn directory_entries(path: &Path) -> Result<BTreeMap<OsString, PathBuf>, SourceImportStorageError> {
    let mut entries = BTreeMap::new();
    for entry in fs::read_dir(path).map_err(|source| inspect_error(path, source))? {
        let entry = entry.map_err(|source| inspect_error(path, source))?;
        entries.insert(entry.file_name(), entry.path());
    }
    Ok(entries)
}

fn inspect_metadata(path: &Path) -> Result<fs::Metadata, SourceImportStorageError> {
    fs::symlink_metadata(path).map_err(|source| inspect_error(path, source))
}

fn read_file(path: &Path) -> Result<Vec<u8>, SourceImportStorageError> {
    fs::read(path).map_err(|source| inspect_error(path, source))
}

fn inspect_error(path: &Path, source: std::io::Error) -> SourceImportStorageError {
    SourceImportStorageError::InspectPath {
        path: path.to_path_buf(),
        source,
    }
}
