use std::fs;
use std::io;
use std::path::Path;

use super::SourceImportStorageError;

pub(super) fn create_dir_all(path: &Path) -> Result<(), SourceImportStorageError> {
    fs::create_dir_all(path).map_err(|source| SourceImportStorageError::CreateDirectory {
        path: path.to_path_buf(),
        source,
    })
}

pub(super) fn write_file(path: &Path, contents: &str) -> Result<(), SourceImportStorageError> {
    fs::write(path, contents).map_err(|source| SourceImportStorageError::WriteFile {
        path: path.to_path_buf(),
        source,
    })
}

pub(super) fn publish_path(from: &Path, to: &Path) -> Result<(), SourceImportStorageError> {
    fs::rename(from, to).map_err(|source| SourceImportStorageError::PublishPath {
        from: from.to_path_buf(),
        to: to.to_path_buf(),
        source,
    })
}

pub(super) fn path_exists(path: &Path) -> Result<bool, SourceImportStorageError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(SourceImportStorageError::InspectPath {
            path: path.to_path_buf(),
            source,
        }),
    }
}

pub(super) fn move_path_if_exists(
    from: &Path,
    to: &Path,
) -> Result<bool, SourceImportStorageError> {
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

pub(super) fn remove_path_if_exists(path: &Path) -> Result<(), SourceImportStorageError> {
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
