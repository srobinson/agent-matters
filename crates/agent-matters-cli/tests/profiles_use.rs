use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::str::contains;
use serde_json::Value;
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

fn native_home_with_codex_auth() -> TempDir {
    let home = TempDir::new().unwrap();
    fs::create_dir_all(home.path().join(".codex")).unwrap();
    fs::write(home.path().join(".codex/auth.json"), br#"{"token":"test"}"#).unwrap();
    home
}

#[test]
fn profiles_use_renders_manual_launch_for_explicit_path() {
    let state = TempDir::new().unwrap();
    let workspace = TempDir::new().unwrap();
    let home = native_home_with_codex_auth();

    bin()
        .current_dir(fixture_path("catalogs/valid"))
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .env("HOME", home.path())
        .args([
            "profiles",
            "use",
            "github-researcher",
            workspace.path().to_str().unwrap(),
            "--runtime",
            "codex",
        ])
        .assert()
        .success()
        .stdout(contains("Profile: github-researcher"))
        .stdout(contains("Runtime: codex"))
        .stdout(contains("Fingerprint: fnv64:"))
        .stdout(contains("Runtime home:"))
        .stdout(contains("Launch environment:"))
        .stdout(contains("CODEX_HOME="))
        .stdout(contains("Manual launch:"))
        .stdout(contains("codex -C"))
        .stdout(contains("Blockers:"))
        .stdout(contains("Warnings:"));

    assert!(
        state
            .path()
            .join("runtimes/github-researcher/codex")
            .exists()
    );
}

#[test]
fn profiles_use_defaults_path_to_cwd() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let home = native_home_with_codex_auth();
    let expected = fs::canonicalize(repo.path()).unwrap().display().to_string();

    let output = bin()
        .current_dir(repo.path())
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .env("HOME", home.path())
        .args(["profiles", "use", "github-researcher", "--runtime", "codex"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let output = String::from_utf8(output).unwrap();

    assert!(output.contains("codex -C"));
    assert!(output.contains(&expected));
}

#[test]
fn profiles_use_missing_env_exits_nonzero() {
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
        .args(["profiles", "use", "github-researcher", "--runtime", "codex"])
        .assert()
        .failure()
        .code(1)
        .stdout(contains("Blockers:"))
        .stderr(contains("profile.required-env-missing"));

    assert!(
        !state
            .path()
            .join("runtimes/github-researcher/codex")
            .exists()
    );
}

#[test]
fn profiles_use_ambiguous_runtime_exits_nonzero_without_flag() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    add_claude_support(repo.path());
    set_profile_runtimes(
        repo.path(),
        r#"[runtimes.codex]
enabled = true

[runtimes.claude]
enabled = true
"#,
    );

    bin()
        .current_dir(repo.path())
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .args(["profiles", "use", "github-researcher"])
        .assert()
        .failure()
        .code(1)
        .stderr(contains("profile.runtime.ambiguous-default"))
        .stderr(contains("claude, codex"));
}

#[test]
fn profiles_use_scope_fail_blocks_out_of_scope_path() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let allowed = repo.path().join("allowed");
    fs::create_dir_all(&allowed).unwrap();
    let outside = TempDir::new().unwrap();
    append_profile_scope(
        repo.path(),
        &format!(
            "[scope]\npaths = [\"{}\"]\nenforcement = \"fail\"\n",
            allowed.display()
        ),
    );

    bin()
        .current_dir(repo.path())
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .args([
            "profiles",
            "use",
            "github-researcher",
            outside.path().to_str().unwrap(),
            "--runtime",
            "codex",
        ])
        .assert()
        .failure()
        .code(1)
        .stderr(contains("profile.scope.out-of-scope"));
}

#[test]
fn profiles_use_json_includes_launch_env_and_args() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let workspace = TempDir::new().unwrap();
    let home = native_home_with_codex_auth();

    let output = bin()
        .current_dir(repo.path())
        .env("AGENT_MATTERS_STATE_DIR", state.path())
        .env("HOME", home.path())
        .args([
            "profiles",
            "use",
            "github-researcher",
            workspace.path().to_str().unwrap(),
            "--runtime",
            "codex",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let output: Value = serde_json::from_slice(&output).unwrap();
    let expected_workspace = fs::canonicalize(workspace.path())
        .unwrap()
        .display()
        .to_string();

    assert_eq!(output["profile"], "github-researcher");
    assert_eq!(output["build"]["runtime"], "codex");
    assert!(output["launch"]["env"]["CODEX_HOME"].as_str().is_some());
    assert_eq!(
        output["launch"]["args"],
        serde_json::json!(["codex", "-C", expected_workspace])
    );
}

fn append_requires(repo: &Path, manifest: &str, body: &str) {
    let path = repo.join(manifest);
    let mut updated = fs::read_to_string(&path).unwrap();
    updated.push_str("\n[requires]\n");
    updated.push_str(body);
    fs::write(path, updated).unwrap();
}

fn append_profile_scope(repo: &Path, scope: &str) {
    let path = repo.join("profiles/renamed-profile-dir/manifest.toml");
    let mut updated = fs::read_to_string(&path).unwrap();
    updated.push('\n');
    updated.push_str(scope);
    fs::write(path, updated).unwrap();
}

fn set_profile_runtimes(repo: &Path, runtimes: &str) {
    let path = repo.join("profiles/renamed-profile-dir/manifest.toml");
    let body = fs::read_to_string(&path).unwrap();
    let prefix = body.split("[runtimes.codex]").next().unwrap();
    fs::write(path, format!("{prefix}{runtimes}")).unwrap();
}

fn add_claude_support(repo: &Path) {
    for manifest in [
        "catalog/agents/github-researcher/manifest.toml",
        "catalog/hooks/session-logger/manifest.toml",
        "catalog/instructions/helioy-core/manifest.toml",
        "catalog/mcp/linear/manifest.toml",
        "catalog/runtime-settings/codex-defaults/manifest.toml",
        "catalog/skills/renamed-skill-dir/manifest.toml",
    ] {
        let path = repo.join(manifest);
        let mut updated = fs::read_to_string(&path).unwrap();
        updated.push_str("\n[runtimes.claude]\nsupported = true\n");
        fs::write(path, updated).unwrap();
    }
}
