use agent_matters_capabilities::doctor::{DoctorRequest, DoctorResult, run_doctor};
use agent_matters_core::domain::Diagnostic;
use tempfile::TempDir;

pub(crate) use crate::support::fixtures::copy_dir;
use crate::support::fixtures::fixture_path;
use crate::support::fixtures::valid_catalog_repo;
pub(crate) use crate::support::manifests::{
    add_required_capability, add_required_env, remove_profile_capability,
};

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
    valid_catalog_repo()
}
