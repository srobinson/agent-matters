//! Recursive file tree comparison for capability overlay diffs.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::diff::{CapabilityDiffError, CapabilityDiffFile, CapabilityDiffStatus, diff_file};

pub(super) fn diff_directories(
    base: &Path,
    overlay: &Path,
) -> Result<Vec<CapabilityDiffFile>, CapabilityDiffError> {
    let base_files = collect_files(base)?;
    let overlay_files = collect_files(overlay)?;
    let paths = base_files
        .keys()
        .chain(overlay_files.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut files = Vec::new();

    for path in paths {
        match (base_files.get(&path), overlay_files.get(&path)) {
            (None, Some(overlay_file)) => files.push(diff_file(
                path,
                CapabilityDiffStatus::Added,
                None,
                Some(overlay_file.len),
            )),
            (Some(base_file), None) => files.push(diff_file(
                path,
                CapabilityDiffStatus::Removed,
                Some(base_file.len),
                None,
            )),
            (Some(base_file), Some(overlay_file)) if files_differ(base_file, overlay_file)? => {
                files.push(diff_file(
                    path,
                    CapabilityDiffStatus::Changed,
                    Some(base_file.len),
                    Some(overlay_file.len),
                ));
            }
            _ => {}
        }
    }

    Ok(files)
}

fn collect_files(root: &Path) -> Result<BTreeMap<String, FileEntry>, CapabilityDiffError> {
    let mut files = BTreeMap::new();
    collect_files_inner(root, root, &mut files)?;
    Ok(files)
}

fn collect_files_inner(
    root: &Path,
    current: &Path,
    files: &mut BTreeMap<String, FileEntry>,
) -> Result<(), CapabilityDiffError> {
    for entry in fs::read_dir(current).map_err(|source| CapabilityDiffError::ReadDirectory {
        path: current.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| CapabilityDiffError::ReadDirectory {
            path: current.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| CapabilityDiffError::InspectFile {
                path: path.clone(),
                source,
            })?;
        if file_type.is_dir() {
            collect_files_inner(root, &path, files)?;
        } else if file_type.is_file() {
            let metadata = entry
                .metadata()
                .map_err(|source| CapabilityDiffError::InspectFile {
                    path: path.clone(),
                    source,
                })?;
            files.insert(
                relative_file_path(root, &path),
                FileEntry {
                    path,
                    len: metadata.len(),
                },
            );
        }
    }
    Ok(())
}

fn files_differ(base: &FileEntry, overlay: &FileEntry) -> Result<bool, CapabilityDiffError> {
    if base.len != overlay.len {
        return Ok(true);
    }

    Ok(read_file(&base.path)? != read_file(&overlay.path)?)
}

fn read_file(path: &Path) -> Result<Vec<u8>, CapabilityDiffError> {
    fs::read(path).map_err(|source| CapabilityDiffError::ReadFile {
        path: path.to_path_buf(),
        source,
    })
}

fn relative_file_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[derive(Debug)]
struct FileEntry {
    path: PathBuf,
    len: u64,
}
