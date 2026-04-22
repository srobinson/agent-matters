use std::fs;
use std::io;
use std::path::Path;

use super::paths::{create_dir_symlink, remove_path_if_exists, temp_sibling};

pub(super) fn update_runtime_pointer(pointer_path: &Path, pointer_target: &Path) -> io::Result<()> {
    let parent = pointer_path
        .parent()
        .ok_or_else(|| io::Error::other("runtime pointer has no parent"))?;
    fs::create_dir_all(parent)?;

    let temp_link = temp_sibling(pointer_path, "pointer");
    remove_path_if_exists(&temp_link)?;
    create_dir_symlink(pointer_target, &temp_link)?;

    match fs::rename(&temp_link, pointer_path) {
        Ok(()) => Ok(()),
        Err(source) => {
            remove_path_if_exists(&temp_link)?;
            Err(source)
        }
    }
}
