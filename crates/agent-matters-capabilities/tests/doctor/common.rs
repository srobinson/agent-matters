use std::fs;
use std::path::Path;

use agent_matters_capabilities::doctor::{DoctorRequest, DoctorResult, run_doctor};
use agent_matters_core::domain::Diagnostic;
use tempfile::TempDir;

use crate::support::fixture_path;

pub(super) fn copy_dir(from: &Path, to: &Path) {
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

pub(super) fn code_count(diagnostics: &[Diagnostic], code: &str) -> usize {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == code)
        .count()
}

pub(super) fn run_fixture(relative: &str) -> DoctorResult {
    let state = TempDir::new().unwrap();
    run_doctor(DoctorRequest {
        repo_root: fixture_path(relative),
        user_state_dir: state.path().to_path_buf(),
        native_home_dir: None,
    })
    .unwrap()
}

pub(super) fn doctor_request(repo: &TempDir, state: &TempDir) -> DoctorRequest {
    DoctorRequest {
        repo_root: repo.path().to_path_buf(),
        user_state_dir: state.path().to_path_buf(),
        native_home_dir: None,
    }
}

pub(super) fn valid_repo() -> TempDir {
    let repo = TempDir::new().unwrap();
    copy_dir(&fixture_path("catalogs/valid"), repo.path());
    repo
}

pub(super) fn append_requires(repo: &TempDir, manifest: &str, body: &str) {
    let path = repo.path().join(manifest);
    let mut updated = fs::read_to_string(&path).unwrap();
    updated.push_str("\n[requires]\n");
    updated.push_str(body);
    fs::write(path, updated).unwrap();
}
