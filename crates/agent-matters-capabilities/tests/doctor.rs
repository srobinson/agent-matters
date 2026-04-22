mod support;

use std::fs;
use std::path::Path;

use agent_matters_capabilities::catalog::{
    LoadCatalogIndexRequest, catalog_index_path, load_or_refresh_catalog_index,
};
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
        native_home_dir: None,
    })
    .unwrap()
}

fn doctor_request(repo: &TempDir, state: &TempDir) -> DoctorRequest {
    DoctorRequest {
        repo_root: repo.path().to_path_buf(),
        user_state_dir: state.path().to_path_buf(),
        native_home_dir: None,
    }
}

fn valid_repo() -> TempDir {
    let repo = TempDir::new().unwrap();
    copy_dir(&fixture_path("catalogs/valid"), repo.path());
    repo
}

fn append_requires(repo: &TempDir, manifest: &str, body: &str) {
    let path = repo.path().join(manifest);
    let mut updated = fs::read_to_string(&path).unwrap();
    updated.push_str("\n[requires]\n");
    updated.push_str(body);
    fs::write(path, updated).unwrap();
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
        native_home_dir: None,
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
fn profile_missing_required_capability_is_error() {
    let repo = valid_repo();
    append_requires(
        &repo,
        "catalog/mcp/linear/manifest.toml",
        "capabilities = [\"skill:playwright\"]\n",
    );
    let profile_manifest = repo
        .path()
        .join("profiles/renamed-profile-dir/manifest.toml");
    let updated = fs::read_to_string(&profile_manifest)
        .unwrap()
        .replace("  \"skill:playwright\",\n", "");
    fs::write(&profile_manifest, updated).unwrap();
    let state = TempDir::new().unwrap();

    let result = run_doctor(doctor_request(&repo, &state)).unwrap();

    let missing = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "profile.required-capability-missing")
        .expect("missing profile requirement diagnostic");
    assert_eq!(missing.severity, DiagnosticSeverity::Error);
    assert!(missing.message.contains("mcp:linear"));
    assert!(missing.message.contains("skill:playwright"));
    assert!(result.has_error_diagnostics());
}

#[test]
fn missing_required_env_is_warning_without_values() {
    let repo = valid_repo();
    append_requires(
        &repo,
        "catalog/mcp/linear/manifest.toml",
        "env = [\"AGENT_MATTERS_DOCTOR_TEST_MISSING_ENV\"]\n",
    );
    let state = TempDir::new().unwrap();

    let result = run_doctor(doctor_request(&repo, &state)).unwrap();
    let encoded = serde_json::to_string(&result).unwrap();

    let missing = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "catalog.required-env-missing")
        .expect("missing env diagnostic");
    assert_eq!(missing.severity, DiagnosticSeverity::Warning);
    assert!(
        missing
            .message
            .contains("AGENT_MATTERS_DOCTOR_TEST_MISSING_ENV")
    );
    assert!(!result.has_error_diagnostics());
    assert!(!encoded.contains("secret"));
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

#[test]
fn missing_vendor_record_is_error() {
    let repo = TempDir::new().unwrap();
    copy_dir(&fixture_path("catalogs/imported"), repo.path());
    fs::write(
        repo.path().join("catalog/skills/playwright/SKILL.md"),
        "Playwright skill\n",
    )
    .unwrap();
    fs::remove_dir_all(repo.path().join("vendor/skills.sh/playwright")).unwrap();
    let state = TempDir::new().unwrap();

    let result = run_doctor(doctor_request(&repo, &state)).unwrap();

    let missing = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "catalog.vendor-record-missing")
        .expect("missing vendor record diagnostic");
    assert_eq!(missing.severity, DiagnosticSeverity::Error);
    assert!(missing.message.contains("skill:playwright"));
    assert!(result.has_error_diagnostics());
}

#[test]
fn nested_vendor_record_satisfies_imported_capability() {
    let repo = TempDir::new().unwrap();
    copy_dir(&fixture_path("catalogs/imported"), repo.path());
    fs::write(
        repo.path().join("catalog/skills/playwright/SKILL.md"),
        "Playwright skill\n",
    )
    .unwrap();
    let vendor_dir = repo.path().join("vendor/skills.sh/playwright");
    let nested_dir = vendor_dir.join("archive");
    fs::create_dir_all(&nested_dir).unwrap();
    fs::rename(
        vendor_dir.join("record.json"),
        nested_dir.join("record.json"),
    )
    .unwrap();
    let state = TempDir::new().unwrap();

    let result = run_doctor(doctor_request(&repo, &state)).unwrap();

    assert!(!result.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "catalog.vendor-record-missing"
            || diagnostic.code == "catalog.vendor-record-read-failed"
    }));
    assert!(!result.has_error_diagnostics(), "{:?}", result.diagnostics);
}

#[test]
fn unsafe_vendor_path_is_error_without_reading_outside_vendor_storage() {
    let repo = TempDir::new().unwrap();
    copy_dir(&fixture_path("catalogs/imported"), repo.path());
    fs::write(
        repo.path().join("catalog/skills/playwright/SKILL.md"),
        "Playwright skill\n",
    )
    .unwrap();
    fs::create_dir_all(repo.path().join("outside")).unwrap();
    fs::write(repo.path().join("outside/record.json"), "{}\n").unwrap();
    let manifest = repo.path().join("catalog/skills/playwright/manifest.toml");
    let updated = fs::read_to_string(&manifest)
        .unwrap()
        .replace("locator = \"playwright\"", "locator = \"../../outside\"");
    fs::write(&manifest, updated).unwrap();
    let state = TempDir::new().unwrap();

    let result = run_doctor(doctor_request(&repo, &state)).unwrap();

    let invalid = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "catalog.vendor-record-path-invalid")
        .expect("missing invalid vendor path diagnostic");
    assert_eq!(invalid.severity, DiagnosticSeverity::Error);
    assert!(invalid.message.contains("skill:playwright"));
    assert!(result.has_error_diagnostics());
}

#[test]
fn imported_overlay_missing_provenance_is_error() {
    let repo = TempDir::new().unwrap();
    copy_dir(&fixture_path("catalogs/imported-overlaid"), repo.path());
    fs::write(
        repo.path().join("overlays/skills/playwright/SKILL.md"),
        "Playwright overlay\n",
    )
    .unwrap();
    let overlay_manifest = repo.path().join("overlays/skills/playwright/manifest.toml");
    let updated = fs::read_to_string(&overlay_manifest).unwrap().replace(
        r#"
[origin]
type = "external"
source = "skills.sh"
locator = "playwright"
version = "1.0.0"
"#,
        "",
    );
    fs::write(&overlay_manifest, updated).unwrap();
    let state = TempDir::new().unwrap();

    let result = run_doctor(doctor_request(&repo, &state)).unwrap();

    let missing = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "catalog.import-provenance-missing")
        .expect("missing provenance diagnostic");
    assert_eq!(missing.severity, DiagnosticSeverity::Error);
    assert!(missing.message.contains("skill:playwright"));
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

    let result = run_doctor(doctor_request(&repo, &state)).unwrap();

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

#[test]
fn doctor_result_has_stable_json_shape() {
    let result = run_fixture("catalogs/valid");

    let encoded = serde_json::to_value(&result).unwrap();

    assert_eq!(encoded["catalog"]["capability_count"], 6);
    assert_eq!(encoded["catalog"]["profile_count"], 1);
    assert_eq!(encoded["index"]["status"], "missing");
    assert!(encoded.get("runtimes").is_some());
    assert!(encoded.get("generated_state").is_some());
    assert_eq!(encoded["diagnostics"], serde_json::json!([]));
}

#[test]
fn runtime_adapter_registry_is_reported() {
    let result = run_fixture("catalogs/valid");

    let codex = result
        .runtimes
        .iter()
        .find(|runtime| runtime.id == "codex")
        .expect("codex runtime summary");
    let claude = result
        .runtimes
        .iter()
        .find(|runtime| runtime.id == "claude")
        .expect("claude runtime summary");

    assert!(codex.adapter_available);
    assert!(codex.default_config_valid);
    assert!(claude.adapter_available);
    assert!(claude.default_config_valid);
}
