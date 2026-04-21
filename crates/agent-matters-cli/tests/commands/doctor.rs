use predicates::str::contains;
use tempfile::TempDir;

use crate::common::{bin, fixture_path};

#[test]
fn doctor_human_reports_catalog_integrity_success() {
    let state = TempDir::new().unwrap();

    bin()
        .current_dir(fixture_path("catalogs/valid"))
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .args(["doctor"])
        .assert()
        .success()
        .stdout(contains("Doctor: catalog integrity"))
        .stdout(contains("No issues found"));
}

#[test]
fn doctor_json_reports_structured_diagnostics_and_exit_code() {
    let state = TempDir::new().unwrap();

    bin()
        .current_dir(fixture_path("catalogs/broken"))
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .args(["doctor", "--json"])
        .assert()
        .failure()
        .code(1)
        .stdout(contains("\"diagnostics\""))
        .stdout(contains("\"code\": \"catalog.manifest-invalid\""))
        .stdout(contains("\"code\": \"catalog.manifest-missing\""))
        .stdout(contains("\"code\": \"catalog.unknown-folder\""));
}

#[test]
fn doctor_human_groups_diagnostics_by_severity_and_source() {
    let state = TempDir::new().unwrap();

    bin()
        .current_dir(fixture_path("catalogs/broken"))
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .args(["doctor"])
        .assert()
        .failure()
        .code(1)
        .stdout(contains("Errors:"))
        .stdout(contains("catalog/mcp/bad-toml/manifest.toml"))
        .stdout(contains("catalog.manifest-invalid"))
        .stdout(contains("catalog/skills/missing/manifest.toml"))
        .stdout(contains("catalog.manifest-missing"));
}
