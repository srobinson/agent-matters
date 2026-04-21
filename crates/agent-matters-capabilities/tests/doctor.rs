mod support;

use std::fs;
use std::path::Path;

use agent_matters_capabilities::catalog::{LoadCatalogIndexRequest, load_or_refresh_catalog_index};
use agent_matters_capabilities::doctor::{DoctorIndexStatus, DoctorRequest, run_doctor};
use agent_matters_core::domain::{Diagnostic, DiagnosticSeverity};
use tempfile::TempDir;

use support::fixture_path;

fn copy_dir(from: &Path, to: &Path) {
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

fn code_count(diagnostics: &[Diagnostic], code: &str) -> usize {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == code)
        .count()
}

fn run_fixture(relative: &str) -> agent_matters_capabilities::doctor::DoctorResult {
    let state = TempDir::new().unwrap();
    run_doctor(DoctorRequest {
        repo_root: fixture_path(relative),
        user_state_dir: state.path().to_path_buf(),
    })
    .unwrap()
}

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
fn missing_capability_file_reference_is_error() {
    let repo = TempDir::new().unwrap();
    copy_dir(&fixture_path("catalogs/valid"), repo.path());
    fs::remove_file(
        repo.path()
            .join("catalog/skills/renamed-skill-dir/SKILL.md"),
    )
    .unwrap();
    let state = TempDir::new().unwrap();

    let result = run_doctor(DoctorRequest {
        repo_root: repo.path().to_path_buf(),
        user_state_dir: state.path().to_path_buf(),
    })
    .unwrap();

    let missing = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "catalog.capability-file-missing")
        .expect("missing file diagnostic");
    assert_eq!(missing.severity, DiagnosticSeverity::Error);
    assert!(missing.message.contains("SKILL.md"));
    assert_eq!(
        missing
            .location
            .as_ref()
            .and_then(|location| location.field.as_deref()),
        Some("files.source")
    );
    assert!(result.has_error_diagnostics());
}

#[test]
fn broken_profile_capability_and_instruction_references_are_reported() {
    let repo = TempDir::new().unwrap();
    copy_dir(&fixture_path("catalogs/valid"), repo.path());
    let profile_manifest = repo
        .path()
        .join("profiles/renamed-profile-dir/manifest.toml");
    let updated = fs::read_to_string(&profile_manifest)
        .unwrap()
        .replace("mcp:linear", "mcp:missing")
        .replace("agent:github-researcher", "agent:missing");
    fs::write(&profile_manifest, updated).unwrap();
    let state = TempDir::new().unwrap();

    let result = run_doctor(DoctorRequest {
        repo_root: repo.path().to_path_buf(),
        user_state_dir: state.path().to_path_buf(),
    })
    .unwrap();

    assert_eq!(
        code_count(&result.diagnostics, "profile.capability-not-found"),
        1
    );
    assert_eq!(
        code_count(&result.diagnostics, "profile.instruction-not-found"),
        1
    );
    assert!(result.has_error_diagnostics());
}

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

    let result = run_doctor(DoctorRequest {
        repo_root: repo.path().to_path_buf(),
        user_state_dir: state.path().to_path_buf(),
    })
    .unwrap();

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
fn doctor_result_has_stable_json_shape() {
    let result = run_fixture("catalogs/valid");

    let encoded = serde_json::to_value(&result).unwrap();

    assert_eq!(encoded["catalog"]["capability_count"], 6);
    assert_eq!(encoded["catalog"]["profile_count"], 1);
    assert_eq!(encoded["index"]["status"], "missing");
    assert_eq!(encoded["diagnostics"], serde_json::json!([]));
}
