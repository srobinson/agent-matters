use std::collections::BTreeMap;
use std::fs;

use agent_matters_capabilities::profiles::{
    BuildProfilePlanRequest, CompileProfileBuildRequest, ProfileBuildWriteStatus,
    compile_profile_build, plan_profile_build,
};
use agent_matters_core::domain::DiagnosticSeverity;
use tempfile::TempDir;

use crate::common::{compile, compile_request, native_home_with_codex_auth, valid_repo};

#[test]
fn compile_reuses_existing_build_for_same_fingerprint() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let first = compile(repo.path(), state.path()).build.unwrap();
    let sentinel = first.build_dir.join("sentinel.txt");
    fs::write(&sentinel, "keep").unwrap();

    let second = compile(repo.path(), state.path()).build.unwrap();

    assert_eq!(second.status, ProfileBuildWriteStatus::Reused);
    assert_eq!(first.build_dir, second.build_dir);
    assert_eq!(fs::read_to_string(sentinel).unwrap(), "keep");
    assert_eq!(
        fs::read_link(&second.runtime_pointer).unwrap(),
        second.pointer_target
    );
}

#[test]
fn compile_rejects_existing_build_with_mismatched_metadata() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let first = compile(repo.path(), state.path()).build.unwrap();
    fs::write(&first.build_plan_path, "{}\n").unwrap();

    let result = compile_profile_build(compile_request(repo.path(), state.path())).unwrap();

    assert!(result.build.is_none());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].severity, DiagnosticSeverity::Error);
    assert_eq!(result.diagnostics[0].code, "profile.build.write-failed");
    assert!(
        result.diagnostics[0]
            .message
            .contains("existing build plan metadata does not match requested plan")
    );
}

#[test]
fn compile_updates_pointer_when_fingerprint_changes() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let first = compile(repo.path(), state.path()).build.unwrap();

    fs::write(
        repo.path()
            .join("catalog/instructions/helioy-core/AGENTS.md"),
        "# Helioy Core\n\nUpdated instruction content.\n",
    )
    .unwrap();
    let second = compile(repo.path(), state.path()).build.unwrap();

    assert_eq!(second.status, ProfileBuildWriteStatus::Created);
    assert_ne!(first.build_id, second.build_id);
    assert!(first.build_dir.is_dir());
    assert!(second.build_dir.is_dir());
    assert_eq!(
        fs::read_link(&second.runtime_pointer).unwrap(),
        second.pointer_target
    );
}

#[test]
fn compile_removes_temp_build_dir_after_pre_rename_write_failure() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    fs::write(repo.path().join("catalog/escape.md"), "escaped\n").unwrap();
    let manifest = repo
        .path()
        .join("catalog/skills/renamed-skill-dir/manifest.toml");
    let updated = fs::read_to_string(&manifest)
        .unwrap()
        .replace("source = \"SKILL.md\"", "source = \"../../escape.md\"");
    fs::write(manifest, updated).unwrap();
    let planned = plan_profile_build(BuildProfilePlanRequest {
        repo_root: repo.path().to_path_buf(),
        user_state_dir: state.path().to_path_buf(),
        profile: "github-researcher".to_string(),
        runtime: Some("codex".to_string()),
    })
    .unwrap();
    assert_eq!(planned.diagnostics, Vec::new());
    let plan = planned.plan.unwrap();
    let build_parent = state
        .path()
        .join(&plan.paths.build_dir)
        .parent()
        .unwrap()
        .to_path_buf();

    let result = compile_profile_build(compile_request(repo.path(), state.path())).unwrap();

    assert!(result.build.is_none());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].severity, DiagnosticSeverity::Error);
    assert_eq!(result.diagnostics[0].code, "profile.build.write-failed");
    assert!(result.diagnostics[0].message.contains("must be relative"));
    let temp_dirs = fs::read_dir(build_parent)
        .unwrap()
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.starts_with('.') && name.contains(".build.tmp-"))
        .collect::<Vec<_>>();
    assert_eq!(temp_dirs, Vec::<String>::new());
}

#[cfg(unix)]
#[test]
fn compile_reports_error_when_state_directory_is_not_writable() {
    use std::os::unix::fs::PermissionsExt;

    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let _ = plan_profile_build(BuildProfilePlanRequest {
        repo_root: repo.path().to_path_buf(),
        user_state_dir: state.path().to_path_buf(),
        profile: "github-researcher".to_string(),
        runtime: Some("codex".to_string()),
    })
    .unwrap();
    let native_home = native_home_with_codex_auth(state.path());

    let original = fs::metadata(state.path()).unwrap().permissions();
    let mut read_only = original.clone();
    read_only.set_mode(0o555);
    fs::set_permissions(state.path(), read_only).unwrap();
    let result = compile_profile_build(CompileProfileBuildRequest {
        repo_root: repo.path().to_path_buf(),
        user_state_dir: state.path().to_path_buf(),
        native_home_dir: Some(native_home),
        profile: "github-researcher".to_string(),
        runtime: Some("codex".to_string()),
        env: BTreeMap::new(),
    })
    .unwrap();
    fs::set_permissions(state.path(), original).unwrap();

    assert!(result.build.is_none());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].severity, DiagnosticSeverity::Error);
    assert_eq!(result.diagnostics[0].code, "profile.build.write-failed");
}
