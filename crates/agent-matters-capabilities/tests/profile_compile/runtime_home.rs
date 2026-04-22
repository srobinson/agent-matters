use std::fs;
use std::path::PathBuf;

use agent_matters_capabilities::profiles::{ProfileBuildWriteStatus, compile_profile_build};
use serde_json::Value;
use tempfile::TempDir;

use crate::common::{
    BuildPathAssertions, PROFILE_MANIFEST, ProfileRuntimeFixture, add_capability_file_mapping,
    compile, compile_request, set_profile_runtimes, valid_repo,
};

#[test]
fn first_compile_creates_immutable_build_and_runtime_pointer() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();

    let build = compile(repo.path(), state.path()).build.unwrap();

    assert_eq!(build.status, ProfileBuildWriteStatus::Created);
    assert!(build.build_dir.is_dir());
    assert!(build.home_dir.is_dir());
    assert_eq!(
        fs::read_link(&build.runtime_pointer).unwrap(),
        build.pointer_target
    );

    let encoded: Value =
        serde_json::from_str(&fs::read_to_string(&build.build_plan_path).unwrap()).unwrap();
    assert_eq!(encoded["profile"], "github-researcher");
    assert_eq!(encoded["runtime"], "codex");
    assert_eq!(encoded["paths"]["home_dir"], build.plan_home_path());
}

#[test]
fn compile_writes_codex_runtime_home_contract() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();

    let build = compile(repo.path(), state.path()).build.unwrap();

    assert!(build.home_dir.join("AGENTS.md").is_file());
    assert_eq!(
        fs::read_to_string(build.home_dir.join("skills/playwright/SKILL.md")).unwrap(),
        "# Playwright\n\nUse Playwright for browser backed workflow verification.\n"
    );
    assert_eq!(
        fs::read_to_string(build.home_dir.join("hooks/session-logger/hook.sh")).unwrap(),
        "#!/usr/bin/env bash\nprintf 'session handover requested\\n'\n"
    );
    assert_eq!(
        fs::read_link(build.home_dir.join("auth.json")).unwrap(),
        state.path().join("native-home/.codex/auth.json")
    );
    assert!(
        fs::symlink_metadata(build.home_dir.join("auth.json"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(build.credential_symlinks.len(), 1);
    assert_eq!(
        build.credential_symlinks[0].source_path.as_deref(),
        Some(state.path().join("native-home/.codex/auth.json").as_path())
    );
    assert_eq!(
        build.credential_symlinks[0].target_path,
        PathBuf::from("auth.json")
    );

    let hooks: Value =
        serde_json::from_str(&fs::read_to_string(build.home_dir.join("hooks.json")).unwrap())
            .unwrap();
    assert_eq!(hooks["hooks"][0]["id"], "hook:session-logger");
    assert_eq!(hooks["hooks"][0]["path"], "hooks/session-logger/hook.sh");

    let config: toml::Value =
        toml::from_str(&fs::read_to_string(build.home_dir.join("config.toml")).unwrap()).unwrap();
    assert_eq!(
        config["mcp_servers"]["linear"]["command"].as_str(),
        Some("linear-mcp")
    );
}

#[test]
fn compile_writes_codex_model_config() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    set_profile_runtimes(
        repo.path(),
        PROFILE_MANIFEST,
        None,
        &[ProfileRuntimeFixture::enabled_with_model(
            "codex", "gpt-5.4",
        )],
    );

    let build = compile(repo.path(), state.path()).build.unwrap();

    assert_eq!(
        fs::read_to_string(build.home_dir.join("config.toml")).unwrap(),
        "model = \"gpt-5.4\"\n\n[mcp_servers.linear]\ncommand = \"linear-mcp\"\n\n[mcp_servers.linear.env]\nLINEAR_API_KEY = \"required\"\n"
    );
}

#[test]
fn compile_reports_unsupported_codex_file_mapping() {
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
        "runtime.codex.file-mapping-unsupported"
    );
}
