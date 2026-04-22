use std::fs;

use tempfile::TempDir;

use crate::common::{plan, valid_repo, write};

#[test]
fn same_resolved_content_has_same_fingerprint_in_different_roots() {
    let first_repo = valid_repo();
    let second_repo = valid_repo();
    let first_state = TempDir::new().unwrap();
    let second_state = TempDir::new().unwrap();

    let first = plan(first_repo.path(), first_state.path());
    let second = plan(second_repo.path(), second_state.path());

    assert_eq!(first.fingerprint, second.fingerprint);
    assert_eq!(first.build_id, second.build_id);
    assert_eq!(first.paths, second.paths);
}

#[test]
fn fingerprint_changes_on_profile_manifest_change() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let first = plan(repo.path(), state.path());

    let manifest_path = repo
        .path()
        .join("profiles/renamed-profile-dir/manifest.toml");
    let updated = fs::read_to_string(&manifest_path)
        .unwrap()
        .replace("Focused research agent", "Focused issue research agent");
    fs::write(manifest_path, updated).unwrap();

    let second = plan(repo.path(), state.path());

    assert_ne!(first.fingerprint, second.fingerprint);
}

#[test]
fn fingerprint_changes_on_included_instruction_file_change() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let first = plan(repo.path(), state.path());

    write(
        repo.path(),
        "catalog/instructions/helioy-core/AGENTS.md",
        "# Helioy Core\n\nUpdated instruction content.\n",
    );
    let second = plan(repo.path(), state.path());

    assert_ne!(first.fingerprint, second.fingerprint);
}

#[test]
fn fingerprint_excludes_unrelated_environment_values() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let first = plan(repo.path(), state.path());

    unsafe {
        std::env::set_var("AGENT_MATTERS_UNRELATED_TEST_ENV", "changed");
    }
    let second = plan(repo.path(), state.path());
    unsafe {
        std::env::remove_var("AGENT_MATTERS_UNRELATED_TEST_ENV");
    }

    assert_eq!(first.fingerprint, second.fingerprint);
}
