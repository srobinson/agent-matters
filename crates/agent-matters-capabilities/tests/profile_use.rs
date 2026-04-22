mod support;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use agent_matters_capabilities::profiles::{UseProfileRequest, use_profile};
use agent_matters_core::domain::{DiagnosticSeverity, ScopeConstraints, ScopeEnforcement};
use serde_json::json;
use tempfile::TempDir;

use support::fixtures::valid_catalog_repo;
use support::manifests::{
    ProfileRuntimeFixture, add_required_env, set_capability_runtime_support, set_profile_runtimes,
    set_profile_scope,
};
use support::native_home::native_home_with_codex_auth;

fn valid_repo() -> TempDir {
    valid_catalog_repo()
}

const PROFILE_MANIFEST: &str = "profiles/renamed-profile-dir/manifest.toml";

fn use_request(repo: &Path, state: &Path, workspace: &Path) -> UseProfileRequest {
    UseProfileRequest {
        repo_root: repo.to_path_buf(),
        user_state_dir: state.to_path_buf(),
        native_home_dir: Some(native_home_with_codex_auth(state)),
        profile: "github-researcher".to_string(),
        runtime: Some("codex".to_string()),
        workspace_path: Some(workspace.to_path_buf()),
        env: BTreeMap::new(),
    }
}

#[test]
fn use_profile_writes_runtime_home_and_launches_explicit_path() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let workspace = TempDir::new().unwrap();

    let result = use_profile(use_request(repo.path(), state.path(), workspace.path())).unwrap();

    assert!(!result.has_error_diagnostics());
    let build = result.build.as_ref().unwrap();
    let launch = result.launch.as_ref().unwrap();
    assert_eq!(
        launch.env.get("CODEX_HOME"),
        Some(&build.runtime_pointer.display().to_string())
    );
    assert_eq!(
        launch.args,
        vec![
            "codex".to_string(),
            "-C".to_string(),
            fs::canonicalize(workspace.path())
                .unwrap()
                .display()
                .to_string()
        ]
    );
    assert!(build.runtime_pointer.exists());
}

#[test]
fn use_profile_launch_instructions_have_stable_json_shape() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let workspace = TempDir::new().unwrap();

    let result = use_profile(use_request(repo.path(), state.path(), workspace.path())).unwrap();
    let build = result.build.as_ref().unwrap();
    let launch = result.launch.as_ref().unwrap();
    let workspace = fs::canonicalize(workspace.path())
        .unwrap()
        .display()
        .to_string();
    let runtime_home = build.runtime_pointer.display().to_string();
    let command = format!(
        "CODEX_HOME={} codex -C {}",
        build.runtime_pointer.display(),
        workspace
    );

    assert_eq!(
        serde_json::to_value(launch).unwrap(),
        json!({
            "env": {
                "CODEX_HOME": runtime_home,
            },
            "args": ["codex", "-C", workspace],
            "command": command,
        })
    );
}

#[test]
fn use_profile_missing_required_env_blocks_build() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let workspace = TempDir::new().unwrap();
    add_required_env(
        repo.path(),
        "catalog/mcp/linear/manifest.toml",
        "LINEAR_API_KEY",
    );

    let result = use_profile(use_request(repo.path(), state.path(), workspace.path())).unwrap();

    assert!(result.build.is_none());
    assert!(result.has_error_diagnostics());
    assert_eq!(result.diagnostics[0].severity, DiagnosticSeverity::Error);
    assert_eq!(result.diagnostics[0].code, "profile.required-env-missing");
}

#[test]
fn use_profile_scope_fail_blocks_out_of_scope_path() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let allowed = repo.path().join("allowed");
    fs::create_dir_all(&allowed).unwrap();
    let outside = TempDir::new().unwrap();
    set_profile_scope(
        repo.path(),
        PROFILE_MANIFEST,
        ScopeConstraints {
            paths: vec![allowed.display().to_string()],
            github_repos: Vec::new(),
            enforcement: ScopeEnforcement::Fail,
        },
    );

    let result = use_profile(use_request(repo.path(), state.path(), outside.path())).unwrap();

    assert!(result.build.is_none());
    assert!(result.has_error_diagnostics());
    assert_eq!(result.diagnostics[0].code, "profile.scope.out-of-scope");
}

#[test]
fn use_profile_scope_fail_without_targets_blocks_build() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let workspace = TempDir::new().unwrap();
    set_profile_scope(
        repo.path(),
        PROFILE_MANIFEST,
        ScopeConstraints {
            paths: Vec::new(),
            github_repos: Vec::new(),
            enforcement: ScopeEnforcement::Fail,
        },
    );

    let result = use_profile(use_request(repo.path(), state.path(), workspace.path())).unwrap();

    assert!(result.build.is_none());
    assert!(result.has_error_diagnostics());
    assert_eq!(result.diagnostics[0].severity, DiagnosticSeverity::Error);
    assert_eq!(result.diagnostics[0].code, "profile.scope.missing-targets");
}

#[test]
fn use_profile_ambiguous_runtime_fails_without_flag() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let workspace = TempDir::new().unwrap();
    add_claude_support(repo.path());
    set_profile_runtimes(
        repo.path(),
        PROFILE_MANIFEST,
        None,
        &[
            ProfileRuntimeFixture::enabled("codex"),
            ProfileRuntimeFixture::enabled("claude"),
        ],
    );
    let mut request = use_request(repo.path(), state.path(), workspace.path());
    request.runtime = None;

    let result = use_profile(request).unwrap();

    assert!(result.build.is_none());
    assert!(result.has_error_diagnostics());
    assert_eq!(
        result.diagnostics[0].code,
        "profile.runtime.ambiguous-default"
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
