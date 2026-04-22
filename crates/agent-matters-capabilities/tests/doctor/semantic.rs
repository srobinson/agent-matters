use std::fs;

use agent_matters_capabilities::doctor::run_doctor;
use agent_matters_core::domain::DiagnosticSeverity;
use tempfile::TempDir;

use crate::common::{append_requires, code_count, copy_dir, doctor_request, valid_repo};
use crate::support::fixture_path;

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

    let result = run_doctor(doctor_request(&repo, &state)).unwrap();

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

#[cfg(unix)]
#[test]
fn symlinked_vendor_path_escape_is_error_without_reading_outside_vendor_storage() {
    use std::os::unix::fs::symlink;

    let repo = TempDir::new().unwrap();
    copy_dir(&fixture_path("catalogs/imported"), repo.path());
    fs::write(
        repo.path().join("catalog/skills/playwright/SKILL.md"),
        "Playwright skill\n",
    )
    .unwrap();
    let vendor_source = repo.path().join("vendor/skills.sh");
    let outside = repo.path().join("outside");
    fs::create_dir_all(&vendor_source).unwrap();
    fs::create_dir_all(outside.join("playwright")).unwrap();
    fs::write(outside.join("playwright/record.json"), "{}\n").unwrap();
    symlink(&outside, vendor_source.join("escaped")).unwrap();
    let manifest = repo.path().join("catalog/skills/playwright/manifest.toml");
    let updated = fs::read_to_string(&manifest).unwrap().replace(
        "locator = \"playwright\"",
        "locator = \"escaped/playwright\"",
    );
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
