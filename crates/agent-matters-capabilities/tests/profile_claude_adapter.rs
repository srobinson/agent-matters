mod support;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use agent_matters_capabilities::profiles::{
    CompileProfileBuildRequest, ProfileBuildWriteStatus, UseProfileRequest, compile_profile_build,
    use_profile,
};
use agent_matters_core::domain::DiagnosticSeverity;
use serde_json::{Value, json};
use tempfile::TempDir;

use support::fixtures::valid_catalog_repo;
use support::manifests::{
    ProfileRuntimeFixture, add_capability_file_mapping, set_capability_runtime_support,
    set_profile_runtimes,
};
use support::native_home::native_home_with_claude_auth;

const PROFILE_MANIFEST: &str = "profiles/renamed-profile-dir/manifest.toml";

fn valid_repo() -> TempDir {
    let repo = valid_catalog_repo();
    add_claude_support(repo.path());
    set_profile_runtimes(
        repo.path(),
        PROFILE_MANIFEST,
        None,
        &[ProfileRuntimeFixture::enabled("claude")],
    );
    repo
}

fn compile_request(repo_root: &Path, state: &Path) -> CompileProfileBuildRequest {
    CompileProfileBuildRequest {
        repo_root: repo_root.to_path_buf(),
        user_state_dir: state.to_path_buf(),
        native_home_dir: Some(native_home_with_claude_auth(state)),
        profile: "github-researcher".to_string(),
        runtime: Some("claude".to_string()),
        env: BTreeMap::new(),
    }
}

fn use_request(repo: &Path, state: &Path, workspace: &Path) -> UseProfileRequest {
    UseProfileRequest {
        repo_root: repo.to_path_buf(),
        user_state_dir: state.to_path_buf(),
        native_home_dir: Some(native_home_with_claude_auth(state)),
        profile: "github-researcher".to_string(),
        runtime: Some("claude".to_string()),
        workspace_path: Some(workspace.to_path_buf()),
        env: BTreeMap::new(),
    }
}

#[test]
fn compile_writes_claude_runtime_home_contract() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();

    let result = compile_profile_build(compile_request(repo.path(), state.path())).unwrap();
    assert_eq!(result.diagnostics, Vec::new());
    let build = result.build.unwrap();

    assert!(build.home_dir.join("CLAUDE.md").is_file());
    assert_eq!(
        fs::read_to_string(build.home_dir.join("skills/playwright/SKILL.md")).unwrap(),
        "# Playwright\n\nUse Playwright for browser backed workflow verification.\n"
    );
    assert!(build.home_dir.join("agents/github-researcher.md").is_file());
    assert_eq!(
        fs::read_to_string(build.home_dir.join("hooks/session-logger/hook.sh")).unwrap(),
        "#!/usr/bin/env bash\nprintf 'session handover requested\\n'\n"
    );
    assert_eq!(
        fs::read_link(build.home_dir.join(".credentials.json")).unwrap(),
        state.path().join("native-home/.claude/.credentials.json")
    );
    assert!(
        fs::symlink_metadata(build.home_dir.join(".credentials.json"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(build.credential_symlinks.len(), 1);
    assert_eq!(
        build.credential_symlinks[0].source_path.as_deref(),
        Some(
            state
                .path()
                .join("native-home/.claude/.credentials.json")
                .as_path()
        )
    );
    assert_eq!(
        build.credential_symlinks[0].target_path,
        PathBuf::from(".credentials.json")
    );

    let config: Value =
        serde_json::from_str(&fs::read_to_string(build.home_dir.join(".claude.json")).unwrap())
            .unwrap();
    assert_eq!(config["mcpServers"]["linear"]["command"], "linear-mcp");
    assert_eq!(config["mcpServers"]["linear"]["args"], json!([]));

    let settings: Value =
        serde_json::from_str(&fs::read_to_string(build.home_dir.join("settings.json")).unwrap())
            .unwrap();
    assert_eq!(
        settings["hooks"]["SessionEnd"][0]["hooks"][0]["command"],
        "\"$CLAUDE_CONFIG_DIR\"/hooks/session-logger/hook.sh"
    );
}

#[test]
fn compile_writes_claude_model_settings() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    set_profile_runtimes(
        repo.path(),
        PROFILE_MANIFEST,
        None,
        &[ProfileRuntimeFixture::enabled_with_model(
            "claude",
            "claude-sonnet-4.5",
        )],
    );

    let result = compile_profile_build(compile_request(repo.path(), state.path())).unwrap();
    assert_eq!(result.diagnostics, Vec::new());
    let settings: Value = serde_json::from_str(
        &fs::read_to_string(result.build.unwrap().home_dir.join("settings.json")).unwrap(),
    )
    .unwrap();

    assert_eq!(settings["model"], "claude-sonnet-4.5");
}

#[test]
fn compile_reuses_claude_build_after_runtime_metadata_update() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();

    let result = compile_profile_build(compile_request(repo.path(), state.path())).unwrap();
    assert_eq!(result.diagnostics, Vec::new());
    let build = result.build.unwrap();
    let config_path = build.home_dir.join(".claude.json");
    let mut config: Value =
        serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
    config["firstStartTime"] = json!("2026-04-21T00:00:00.000Z");
    config["migrationVersion"] = json!(11);
    config["userID"] = json!("claude-runtime-owned");
    fs::write(
        &config_path,
        format!("{}\n", serde_json::to_string_pretty(&config).unwrap()),
    )
    .unwrap();

    let result = compile_profile_build(compile_request(repo.path(), state.path())).unwrap();

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(
        result.build.unwrap().status,
        ProfileBuildWriteStatus::Reused
    );
}

#[test]
fn compile_warns_when_claude_auth_source_is_missing() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let request = compile_request(repo.path(), state.path());
    let native_home = request.native_home_dir.as_ref().unwrap();
    fs::remove_file(native_home.join(".claude/.credentials.json")).unwrap();

    let result = compile_profile_build(request).unwrap();

    assert!(result.build.is_some());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].severity, DiagnosticSeverity::Warning);
    assert_eq!(
        result.diagnostics[0].code,
        "runtime.credential-source-missing"
    );
    assert!(matches!(
        fs::symlink_metadata(result.build.unwrap().home_dir.join(".credentials.json")),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound
    ));
}

#[test]
fn compile_reports_unsupported_claude_file_mapping() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    fs::write(
        repo.path()
            .join("catalog/skills/renamed-skill-dir/README.md"),
        "extra\n",
    )
    .unwrap();
    add_capability_file_mapping(
        repo.path(),
        "catalog/skills/renamed-skill-dir/manifest.toml",
        "readme",
        "README.md",
    );

    let result = compile_profile_build(compile_request(repo.path(), state.path())).unwrap();

    assert!(result.build.is_none());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].code,
        "runtime.claude.file-mapping-unsupported"
    );
}

#[test]
fn use_profile_launch_instructions_have_claude_shape() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let workspace = TempDir::new().unwrap();

    let result = use_profile(use_request(repo.path(), state.path(), workspace.path())).unwrap();

    assert!(!result.has_error_diagnostics());
    let build = result.build.as_ref().unwrap();
    let launch = result.launch.as_ref().unwrap();
    let workspace = fs::canonicalize(workspace.path())
        .unwrap()
        .display()
        .to_string();
    let runtime_home = build.runtime_pointer.display().to_string();

    assert_eq!(launch.env.get("CLAUDE_CONFIG_DIR"), Some(&runtime_home));
    assert_eq!(launch.args, vec!["claude".to_string(), workspace.clone()]);
    assert_eq!(
        launch.command,
        format!("CLAUDE_CONFIG_DIR={runtime_home} claude {workspace}")
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
