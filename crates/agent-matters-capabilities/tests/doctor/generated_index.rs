use std::fs;

use agent_matters_capabilities::catalog::{
    LoadCatalogIndexRequest, catalog_index_path, load_or_refresh_catalog_index,
};
use agent_matters_capabilities::doctor::{DoctorIndexStatus, run_doctor};
use agent_matters_core::domain::DiagnosticSeverity;
use tempfile::TempDir;

use crate::common::{copy_dir, doctor_request, valid_repo};
use crate::support::fixtures::fixture_path;

#[test]
fn stale_generated_index_is_warning_only() {
    let repo = TempDir::new().unwrap();
    copy_dir(&fixture_path("catalogs/valid"), repo.path());
    let state = TempDir::new().unwrap();
    load_or_refresh_catalog_index(LoadCatalogIndexRequest {
        repo_root: repo.path().to_path_buf(),
        user_state_dir: state.path().to_path_buf(),
    })
    .unwrap();
    let profile_manifest = repo
        .path()
        .join("profiles/renamed-profile-dir/manifest.toml");
    let updated = fs::read_to_string(&profile_manifest)
        .unwrap()
        .replace("Focused research agent", "Changed research agent");
    fs::write(&profile_manifest, updated).unwrap();

    let result = run_doctor(doctor_request(&repo, &state)).unwrap();

    assert_eq!(result.index.status, DoctorIndexStatus::Stale);
    let stale = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "catalog.index-stale")
        .expect("stale index diagnostic");
    assert_eq!(stale.severity, DiagnosticSeverity::Warning);
    assert!(!result.has_error_diagnostics());
}

#[test]
fn corrupt_generated_index_is_error() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let index_path = catalog_index_path(state.path());
    fs::create_dir_all(index_path.parent().unwrap()).unwrap();
    fs::write(&index_path, "{not valid json").unwrap();

    let result = run_doctor(doctor_request(&repo, &state)).unwrap();

    assert_eq!(result.index.status, DoctorIndexStatus::Corrupt);
    let corrupt = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "catalog.index-corrupt")
        .expect("corrupt index diagnostic");
    assert_eq!(corrupt.severity, DiagnosticSeverity::Error);
    assert!(result.has_error_diagnostics());
}

#[test]
fn unreadable_generated_index_is_error() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let index_path = catalog_index_path(state.path());
    fs::create_dir_all(&index_path).unwrap();

    let result = run_doctor(doctor_request(&repo, &state)).unwrap();

    assert_eq!(result.index.status, DoctorIndexStatus::ReadFailed);
    let read_failed = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "catalog.index-read-failed")
        .expect("read failed index diagnostic");
    assert_eq!(read_failed.severity, DiagnosticSeverity::Error);
    assert!(result.has_error_diagnostics());
}
