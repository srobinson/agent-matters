use std::path::PathBuf;

use super::fs::{move_path_if_exists, path_exists, publish_path, remove_path_if_exists};
use super::partial::{PublishedTree, complete_partial_new_import, trees_match};
use super::paths::{ImportTreePaths, durable_sibling};
use super::{SourceImportStorageError, WriteSourceImportStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReplacementBackupPaths {
    capability_dir: PathBuf,
    vendor_dir: PathBuf,
}

pub(super) fn publish_staged_import(
    staging_paths: &ImportTreePaths,
    final_paths: &ImportTreePaths,
    replace_existing: bool,
) -> Result<WriteSourceImportStatus, SourceImportStorageError> {
    if replace_existing {
        return publish_replacing_staged_import(staging_paths, final_paths);
    }

    publish_new_staged_import(staging_paths, final_paths)
}

fn publish_new_staged_import(
    staging_paths: &ImportTreePaths,
    final_paths: &ImportTreePaths,
) -> Result<WriteSourceImportStatus, SourceImportStorageError> {
    match (
        path_exists(&final_paths.capability_dir)?,
        path_exists(&final_paths.vendor_dir)?,
    ) {
        (false, false) => {
            publish_fresh_new_import(staging_paths, final_paths)?;
            Ok(WriteSourceImportStatus::Created)
        }
        (true, false) => {
            complete_partial_new_import(PublishedTree::Capability, staging_paths, final_paths)?;
            Ok(WriteSourceImportStatus::Created)
        }
        (false, true) => {
            complete_partial_new_import(PublishedTree::Vendor, staging_paths, final_paths)?;
            Ok(WriteSourceImportStatus::Created)
        }
        (true, true) if complete_import_matches(staging_paths, final_paths)? => {
            discard_staged_import(staging_paths)?;
            Ok(WriteSourceImportStatus::Unchanged)
        }
        (true, true) => Err(SourceImportStorageError::AlreadyExists {
            path: final_paths.capability_dir.clone(),
        }),
    }
}

fn publish_fresh_new_import(
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
) -> Result<WriteSourceImportStatus, SourceImportStorageError> {
    let backup_paths = ReplacementBackupPaths {
        capability_dir: durable_sibling(&final_paths.capability_dir, "source-import-backup"),
        vendor_dir: durable_sibling(&final_paths.vendor_dir, "source-import-backup"),
    };
    recover_interrupted_replacement(&backup_paths, final_paths)?;

    if path_exists(&final_paths.capability_dir)? && path_exists(&final_paths.vendor_dir)? {
        if complete_import_matches(staging_paths, final_paths)? {
            discard_staged_import(staging_paths)?;
            return Ok(WriteSourceImportStatus::Unchanged);
        }
    } else {
        remove_path_if_exists(&final_paths.capability_dir)?;
        remove_path_if_exists(&final_paths.vendor_dir)?;
        publish_fresh_new_import(staging_paths, final_paths)?;
        return Ok(WriteSourceImportStatus::Created);
    }

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

    match publish_fresh_new_import(staging_paths, final_paths) {
        Ok(()) => {
            cleanup_replacement_backups(&backup_paths);
            Ok(WriteSourceImportStatus::Updated)
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

fn complete_import_matches(
    staging_paths: &ImportTreePaths,
    final_paths: &ImportTreePaths,
) -> Result<bool, SourceImportStorageError> {
    Ok(
        trees_match(&staging_paths.capability_dir, &final_paths.capability_dir)?
            && trees_match(&staging_paths.vendor_dir, &final_paths.vendor_dir)?,
    )
}

fn discard_staged_import(paths: &ImportTreePaths) -> Result<(), SourceImportStorageError> {
    remove_path_if_exists(&paths.capability_dir)?;
    remove_path_if_exists(&paths.vendor_dir)
}

fn recover_interrupted_replacement(
    backup_paths: &ReplacementBackupPaths,
    final_paths: &ImportTreePaths,
) -> Result<(), SourceImportStorageError> {
    let capability_backup_exists = path_exists(&backup_paths.capability_dir)?;
    let vendor_backup_exists = path_exists(&backup_paths.vendor_dir)?;
    if !capability_backup_exists && !vendor_backup_exists {
        return Ok(());
    }

    let capability_final_exists = path_exists(&final_paths.capability_dir)?;
    let vendor_final_exists = path_exists(&final_paths.vendor_dir)?;
    if capability_final_exists && vendor_final_exists {
        return remove_replacement_backups(backup_paths);
    }

    restore_interrupted_replacement(
        backup_paths,
        final_paths,
        capability_backup_exists,
        vendor_backup_exists,
    )
}

fn restore_interrupted_replacement(
    backup_paths: &ReplacementBackupPaths,
    final_paths: &ImportTreePaths,
    capability_backed_up: bool,
    vendor_backed_up: bool,
) -> Result<(), SourceImportStorageError> {
    if !capability_backed_up && !path_exists(&final_paths.capability_dir)? {
        return Err(SourceImportStorageError::ReplacementRecoveryConflict {
            missing: final_paths.capability_dir.clone(),
            backup: backup_paths.capability_dir.clone(),
        });
    }
    if !vendor_backed_up && !path_exists(&final_paths.vendor_dir)? {
        return Err(SourceImportStorageError::ReplacementRecoveryConflict {
            missing: final_paths.vendor_dir.clone(),
            backup: backup_paths.vendor_dir.clone(),
        });
    }

    if capability_backed_up {
        remove_path_if_exists(&final_paths.capability_dir)?;
        publish_path(&backup_paths.capability_dir, &final_paths.capability_dir)?;
    }
    if vendor_backed_up {
        remove_path_if_exists(&final_paths.vendor_dir)?;
        publish_path(&backup_paths.vendor_dir, &final_paths.vendor_dir)?;
    }
    Ok(())
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

fn remove_replacement_backups(
    paths: &ReplacementBackupPaths,
) -> Result<(), SourceImportStorageError> {
    remove_path_if_exists(&paths.capability_dir)?;
    remove_path_if_exists(&paths.vendor_dir)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use agent_matters_core::catalog::MANIFEST_FILE_NAME;
    use tempfile::TempDir;

    use super::super::SourceImportStorageError;
    use super::super::paths::ImportTreePaths;
    use super::*;

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
