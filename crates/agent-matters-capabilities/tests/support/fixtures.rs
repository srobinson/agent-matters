use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

/// Absolute path to `crates/agent-matters-capabilities/tests/fixtures/`.
///
/// Derived from `CARGO_MANIFEST_DIR`, which cargo sets to the path of the
/// crate whose tests are running. Stable across invocation directory.
pub(crate) fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

/// Resolve a path relative to the fixtures root. Panics if the resulting
/// path does not exist so a typo in a fixture reference fails loudly.
pub(crate) fn fixture_path(relative: impl AsRef<Path>) -> PathBuf {
    let full = fixtures_root().join(relative.as_ref());
    assert!(
        full.exists(),
        "fixture `{}` does not exist (resolved to {})",
        relative.as_ref().display(),
        full.display()
    );
    full
}

pub(crate) fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let source = entry.path();
        let target = to.join(entry.file_name());
        if source.is_dir() {
            copy_dir(&source, &target);
        } else {
            fs::copy(&source, &target).unwrap();
        }
    }
}

pub(crate) fn valid_catalog_repo() -> TempDir {
    let repo = TempDir::new().unwrap();
    copy_dir(&fixture_path("catalogs/valid"), repo.path());
    repo
}
