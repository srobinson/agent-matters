use std::fs;
use std::path::Path;

use agent_matters_core::manifest::ScopeEnforcement;
use predicates::str::contains;
use serde_json::Value;
use tempfile::TempDir;

#[path = "support/mod.rs"]
mod common;

use common::{
    add_required_env, bin, fixture_path, native_home_with_codex_auth,
    set_capability_runtime_support, set_profile_path_scope, set_profile_runtimes,
    valid_catalog_repo,
};

const GITHUB_RESEARCHER_PROFILE: &str = "profiles/renamed-profile-dir/manifest.toml";

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
    let repo = valid_catalog_repo();
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
    let repo = valid_catalog_repo();
    let state = TempDir::new().unwrap();
    add_required_env(
        repo.path(),
        "catalog/mcp/linear/manifest.toml",
        "LINEAR_API_KEY",
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
    let repo = valid_catalog_repo();
    let state = TempDir::new().unwrap();
    add_claude_support(repo.path());
    set_profile_runtimes(
        repo.path(),
        GITHUB_RESEARCHER_PROFILE,
        &[("codex", true), ("claude", true)],
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
    let repo = valid_catalog_repo();
    let state = TempDir::new().unwrap();
    let allowed = repo.path().join("allowed");
    fs::create_dir_all(&allowed).unwrap();
    let outside = TempDir::new().unwrap();
    set_profile_path_scope(
        repo.path(),
        GITHUB_RESEARCHER_PROFILE,
        vec![allowed.display().to_string()],
        ScopeEnforcement::Fail,
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
    let repo = valid_catalog_repo();
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

fn add_claude_support(repo: &Path) {
    for manifest in [
        "catalog/agents/github-researcher/manifest.toml",
        "catalog/hooks/session-logger/manifest.toml",
        "catalog/instructions/helioy-core/manifest.toml",
        "catalog/mcp/linear/manifest.toml",
        "catalog/runtime-settings/codex-defaults/manifest.toml",
        "catalog/skills/renamed-skill-dir/manifest.toml",
    ] {
        set_capability_runtime_support(repo, manifest, "claude", true);
    }
}
