//! Shared helpers for integration tests. Cargo treats files directly
//! under `tests/` as test binaries; nesting helpers in a subdirectory
//! (here `support/mod.rs`) keeps them out of that contract so they can
//! be shared by `mod support;` in any integration test file.

use std::path::{Path, PathBuf};

/// Absolute path to `crates/agent-matters-capabilities/tests/fixtures/`.
///
/// Derived from `CARGO_MANIFEST_DIR`, which cargo sets to the path of the
/// crate whose tests are running. Stable across invocation directory.
pub fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

/// Resolve a path relative to the fixtures root. Panics if the resulting
/// path does not exist so a typo in a fixture reference fails loudly.
pub fn fixture_path(relative: impl AsRef<Path>) -> PathBuf {
    let full = fixtures_root().join(relative.as_ref());
    assert!(
        full.exists(),
        "fixture `{}` does not exist (resolved to {})",
        relative.as_ref().display(),
        full.display()
    );
    full
}
