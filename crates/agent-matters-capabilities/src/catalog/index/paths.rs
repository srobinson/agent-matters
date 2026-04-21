use std::path::{Path, PathBuf};

use agent_matters_core::catalog::{CATALOG_INDEX_FILE_NAME, INDEXES_DIR_NAME};

pub fn catalog_index_path(user_state_dir: &Path) -> PathBuf {
    user_state_dir
        .join(INDEXES_DIR_NAME)
        .join(CATALOG_INDEX_FILE_NAME)
}

pub(super) fn relative_path(repo_root: &Path, path: &Path) -> String {
    path.strip_prefix(repo_root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}
