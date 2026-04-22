use std::fs;

use agent_matters_capabilities::catalog::catalog_index_path;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use tempfile::TempDir;

use crate::common::{bin, fixture_path};

const CODEX_TEST_TOKEN: &str = "alp_1986_secret_credential_value";

fn native_home_with_codex_auth(root: &TempDir) -> std::path::PathBuf {
    let home = root.path().join("native-home");
    fs::create_dir_all(home.join(".codex")).unwrap();
    fs::write(
        home.join(".codex/auth.json"),
        format!(r#"{{"token":"{CODEX_TEST_TOKEN}"}}"#),
    )
    .unwrap();
    home
}

#[test]
fn doctor_human_reports_catalog_integrity_success() {
    let state = TempDir::new().unwrap();
    let home = native_home_with_codex_auth(&state);

    bin()
        .current_dir(fixture_path("catalogs/valid"))
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .env("HOME", home)
        .args(["doctor"])
        .assert()
        .success()
        .stdout(contains("Doctor: catalog integrity"))
        .stdout(contains("No issues found"));
}

#[test]
fn doctor_human_explains_missing_codex_auth() {
    let state = TempDir::new().unwrap();
    let home = state.path().join("native-home");
    fs::create_dir_all(home.join(".codex")).unwrap();

    bin()
        .current_dir(fixture_path("catalogs/valid"))
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .env("HOME", home)
        .args(["doctor"])
        .assert()
        .success()
        .stdout(contains("Warnings:"))
        .stdout(contains("runtime.credential-source-missing"))
        .stdout(contains("authenticate with `codex`"));
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
fn doctor_json_fails_on_corrupt_generated_index() {
    let state = TempDir::new().unwrap();
    let home = native_home_with_codex_auth(&state);
    let index_path = catalog_index_path(state.path());
    fs::create_dir_all(index_path.parent().unwrap()).unwrap();
    fs::write(&index_path, "{not valid json").unwrap();

    bin()
        .current_dir(fixture_path("catalogs/valid"))
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .env("HOME", home)
        .args(["doctor", "--json"])
        .assert()
        .failure()
        .code(1)
        .stdout(contains("\"status\": \"corrupt\""))
        .stdout(contains("\"severity\": \"error\""))
        .stdout(contains("\"code\": \"catalog.index-corrupt\""));
}

#[test]
fn doctor_human_fails_on_corrupt_generated_index() {
    let state = TempDir::new().unwrap();
    let home = native_home_with_codex_auth(&state);
    let index_path = catalog_index_path(state.path());
    fs::create_dir_all(index_path.parent().unwrap()).unwrap();
    fs::write(&index_path, "{not valid json").unwrap();
    let index_display = index_path.display().to_string();

    bin()
        .current_dir(fixture_path("catalogs/valid"))
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .env("HOME", home)
        .args(["doctor"])
        .assert()
        .failure()
        .code(1)
        .stdout(contains("Index: Corrupt"))
        .stdout(contains("Errors:"))
        .stdout(contains(index_display))
        .stdout(contains("catalog.index-corrupt"))
        .stdout(contains("generated catalog index"))
        .stdout(contains("is corrupt"))
        .stdout(contains(
            "hint: delete the generated index or rerun a catalog command to rebuild it",
        ));
}

#[test]
fn doctor_json_fails_on_unreadable_generated_index() {
    let state = TempDir::new().unwrap();
    let home = native_home_with_codex_auth(&state);
    let index_path = catalog_index_path(state.path());
    fs::create_dir_all(&index_path).unwrap();

    bin()
        .current_dir(fixture_path("catalogs/valid"))
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .env("HOME", home)
        .args(["doctor", "--json"])
        .assert()
        .failure()
        .code(1)
        .stdout(contains("\"status\": \"read-failed\""))
        .stdout(contains("\"severity\": \"error\""))
        .stdout(contains("\"code\": \"catalog.index-read-failed\""));
}

#[test]
fn doctor_human_fails_on_unreadable_generated_index() {
    let state = TempDir::new().unwrap();
    let home = native_home_with_codex_auth(&state);
    let index_path = catalog_index_path(state.path());
    fs::create_dir_all(&index_path).unwrap();
    let index_display = index_path.display().to_string();

    bin()
        .current_dir(fixture_path("catalogs/valid"))
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .env("HOME", home)
        .args(["doctor"])
        .assert()
        .failure()
        .code(1)
        .stdout(contains("Index: ReadFailed"))
        .stdout(contains("Errors:"))
        .stdout(contains(index_display))
        .stdout(contains("catalog.index-read-failed"))
        .stdout(contains("failed to read generated catalog index"))
        .stdout(contains(
            "hint: remove or fix the generated index path, then rerun doctor",
        ));
}

#[cfg(unix)]
#[test]
fn doctor_json_reports_invalid_runtime_pointer() {
    use std::os::unix::fs::symlink;

    let state = TempDir::new().unwrap();
    let home = native_home_with_codex_auth(&state);
    let pointer = state.path().join("runtimes/github-researcher/codex");
    fs::create_dir_all(pointer.parent().unwrap()).unwrap();
    symlink(
        "../../builds/codex/github-researcher/missing/home",
        &pointer,
    )
    .unwrap();

    bin()
        .current_dir(fixture_path("catalogs/valid"))
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .env("HOME", home)
        .args(["doctor", "--json"])
        .assert()
        .failure()
        .code(1)
        .stdout(contains("\"code\": \"runtime.pointer-target-invalid\""))
        .stdout(contains("\"severity\": \"error\""))
        .stdout(contains("github-researcher"))
        .stdout(contains("codex"))
        .stdout(contains("auth.json").not())
        .stdout(contains("\"token\"").not())
        .stdout(contains(CODEX_TEST_TOKEN).not())
        .stderr(contains(CODEX_TEST_TOKEN).not());
}

#[test]
fn doctor_json_fails_when_state_root_parent_is_file() {
    let scratch = TempDir::new().unwrap();
    let home = native_home_with_codex_auth(&scratch);
    let blocker = scratch.path().join("not-a-directory");
    fs::write(&blocker, b"blocks nested state roots").unwrap();

    bin()
        .current_dir(fixture_path("catalogs/valid"))
        .env("AGENT_MATTERS_STATE_DIR", blocker.join("child-state"))
        .env("HOME", home)
        .args(["doctor", "--json"])
        .assert()
        .failure()
        .code(1)
        .stdout(contains(
            "\"code\": \"runtime.state-root-parent-not-directory\"",
        ))
        .stdout(contains("\"severity\": \"error\""))
        .stdout(contains("\"writable\": false"))
        .stdout(contains("\"token\"").not())
        .stdout(contains(CODEX_TEST_TOKEN).not())
        .stderr(contains(CODEX_TEST_TOKEN).not());
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
