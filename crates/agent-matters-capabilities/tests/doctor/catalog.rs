use agent_matters_capabilities::catalog::{LoadCatalogIndexRequest, load_or_refresh_catalog_index};
use agent_matters_capabilities::doctor::{DoctorIndexStatus, DoctorRequest, run_doctor};
use agent_matters_core::domain::DiagnosticSeverity;
use tempfile::TempDir;

use crate::common::{code_count, run_fixture};
use crate::support::fixture_path;

#[test]
fn clean_catalog_with_fresh_index_passes_without_diagnostics() {
    let repo_root = fixture_path("catalogs/valid");
    let state = TempDir::new().unwrap();
    load_or_refresh_catalog_index(LoadCatalogIndexRequest {
        repo_root: repo_root.clone(),
        user_state_dir: state.path().to_path_buf(),
    })
    .unwrap();

    let result = run_doctor(DoctorRequest {
        repo_root,
        user_state_dir: state.path().to_path_buf(),
        native_home_dir: None,
    })
    .unwrap();

    assert_eq!(result.catalog.capability_count, 6);
    assert_eq!(result.catalog.profile_count, 1);
    assert_eq!(result.index.status, DoctorIndexStatus::Fresh);
    assert_eq!(result.diagnostics, Vec::new());
    assert!(!result.has_error_diagnostics());
}

#[test]
fn multiple_broken_manifests_are_all_reported() {
    let result = run_fixture("catalogs/broken");

    assert_eq!(
        code_count(&result.diagnostics, "catalog.manifest-invalid"),
        1
    );
    assert_eq!(
        code_count(&result.diagnostics, "catalog.manifest-missing"),
        1
    );
    assert_eq!(code_count(&result.diagnostics, "catalog.unknown-folder"), 1);
    assert!(result.has_error_diagnostics());
}

#[test]
fn duplicate_capability_id_is_error() {
    let result = run_fixture("catalogs/duplicate-capability");
    let duplicate = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "catalog.duplicate-id")
        .expect("duplicate id diagnostic");

    assert_eq!(duplicate.severity, DiagnosticSeverity::Error);
    assert!(duplicate.message.contains("skill:dupe"));
    assert!(result.has_error_diagnostics());
}

#[test]
fn broken_overlay_target_is_error() {
    let result = run_fixture("catalogs/overlay-target-missing");
    let missing = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "catalog.overlay-target-missing")
        .expect("missing overlay target diagnostic");

    assert_eq!(missing.severity, DiagnosticSeverity::Error);
    assert!(result.has_error_diagnostics());
}
