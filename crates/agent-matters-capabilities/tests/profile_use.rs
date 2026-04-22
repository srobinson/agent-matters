use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use agent_matters_capabilities::profiles::{UseProfileRequest, use_profile};
use agent_matters_core::domain::DiagnosticSeverity;
use serde_json::json;
use tempfile::TempDir;

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

fn fixture_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(relative)
}

fn valid_repo() -> TempDir {
    let repo = TempDir::new().unwrap();
    copy_dir(&fixture_path("catalogs/valid"), repo.path());
    repo
}

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

fn native_home_with_codex_auth(root: &Path) -> PathBuf {
    let home = root.join("native-home");
    fs::create_dir_all(home.join(".codex")).unwrap();
    fs::write(home.join(".codex/auth.json"), br#"{"token":"test"}"#).unwrap();
    home
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
    append_requires(
        repo.path(),
        "catalog/mcp/linear/manifest.toml",
        "env = [\"LINEAR_API_KEY\"]\n",
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
    append_profile_scope(
        repo.path(),
        &format!(
            "[scope]\npaths = [\"{}\"]\nenforcement = \"fail\"\n",
            allowed.display()
        ),
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
    append_profile_scope(repo.path(), "[scope]\nenforcement = \"fail\"\n");

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
        r#"[runtimes.codex]
enabled = true

[runtimes.claude]
enabled = true
"#,
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
