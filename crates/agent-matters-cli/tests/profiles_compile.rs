use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::str::contains;
use tempfile::TempDir;

fn bin() -> Command {
    Command::cargo_bin("agent-matters").expect("cargo bin available in tests")
}

fn fixture_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../agent-matters-capabilities/tests/fixtures")
        .join(relative)
}

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

fn valid_repo() -> TempDir {
    let repo = TempDir::new().unwrap();
    copy_dir(&fixture_path("catalogs/valid"), repo.path());
    repo
}

fn append_requires(repo: &Path, manifest: &str, body: &str) {
    let path = repo.join(manifest);
    let mut updated = fs::read_to_string(&path).unwrap();
    updated.push_str("\n[requires]\n");
    updated.push_str(body);
    fs::write(path, updated).unwrap();
}

#[test]
fn profiles_compile_renders_human_summary_and_writes_runtime_pointer() {
    let state = TempDir::new().unwrap();

    bin()
        .current_dir(fixture_path("catalogs/valid"))
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .args([
            "profiles",
            "compile",
            "github-researcher",
            "--runtime",
            "codex",
        ])
        .assert()
        .success()
        .stdout(contains("Profile: github-researcher"))
        .stdout(contains("Runtime: codex"))
        .stdout(contains("Fingerprint: fnv64:"))
        .stdout(contains("Immutable build path:"))
        .stdout(contains("Stable runtime path:"))
        .stdout(contains("Warnings:"))
        .stdout(contains("none"));

    assert!(
        state
            .path()
            .join("runtimes/github-researcher/codex")
            .exists()
    );
}

#[test]
fn profiles_compile_json_includes_stable_build_shape_without_secret_values() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    append_requires(
        repo.path(),
        "catalog/mcp/linear/manifest.toml",
        "env = [\"LINEAR_API_KEY\"]\n",
    );

    let output = bin()
        .current_dir(repo.path())
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .env("LINEAR_API_KEY", "secret-value-never-rendered")
        .args([
            "profiles",
            "compile",
            "github-researcher",
            "--runtime",
            "codex",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let output = String::from_utf8(output).unwrap();

    assert!(output.contains("\"profile\": \"github-researcher\""));
    assert!(output.contains("\"build\""));
    assert!(output.contains("\"runtime\": \"codex\""));
    assert!(output.contains("\"fingerprint\": \"fnv64:"));
    assert!(output.contains("\"runtime_pointer\""));
    assert!(!output.contains("secret-value-never-rendered"));
}

#[cfg(unix)]
#[test]
fn profiles_compile_tolerates_non_utf8_environment_values() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let state = TempDir::new().unwrap();

    bin()
        .current_dir(fixture_path("catalogs/valid"))
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .env(
            "AGENT_MATTERS_BINARY_VALUE",
            OsString::from_vec(vec![b'o', 0x80, b'k']),
        )
        .args([
            "profiles",
            "compile",
            "github-researcher",
            "--runtime",
            "codex",
        ])
        .assert()
        .success()
        .stdout(contains("Profile: github-researcher"));
}

#[test]
fn profiles_compile_human_output_includes_missing_env_warning() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    append_requires(
        repo.path(),
        "catalog/mcp/linear/manifest.toml",
        "env = [\"LINEAR_API_KEY\"]\n",
    );

    bin()
        .current_dir(repo.path())
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .env_remove("LINEAR_API_KEY")
        .args([
            "profiles",
            "compile",
            "github-researcher",
            "--runtime",
            "codex",
        ])
        .assert()
        .success()
        .stdout(contains("Warnings:"))
        .stdout(contains("profile.required-env-missing"))
        .stderr(contains("warning profile.required-env-missing"));
}

#[test]
fn profiles_compile_missing_required_capability_exits_with_error() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    append_requires(
        repo.path(),
        "catalog/mcp/linear/manifest.toml",
        "capabilities = [\"mcp:context-matters\"]\n",
    );

    bin()
        .current_dir(repo.path())
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .args([
            "profiles",
            "compile",
            "github-researcher",
            "--runtime",
            "codex",
        ])
        .assert()
        .failure()
        .code(1)
        .stderr(contains("profile.required-capability-missing"));
}

#[test]
fn profiles_compile_runtime_incompatibility_exits_with_error() {
    let state = TempDir::new().unwrap();

    bin()
        .current_dir(fixture_path("catalogs/valid"))
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .args([
            "profiles",
            "compile",
            "github-researcher",
            "--runtime",
            "claude",
        ])
        .assert()
        .failure()
        .code(1)
        .stderr(contains("profile.build-plan.runtime-unavailable"));
}
