mod support;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use agent_matters_capabilities::profiles::{
    BuildProfilePlanRequest, CompileProfileBuildRequest, CompileProfileBuildResult,
    ProfileBuildWriteStatus, compile_profile_build, plan_profile_build,
};
use agent_matters_core::domain::DiagnosticSeverity;
use serde_json::Value;
use tempfile::TempDir;

use support::fixture_path;

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
    let tmp = TempDir::new().unwrap();
    copy_dir(&fixture_path("catalogs/valid"), tmp.path());
    tmp
}

fn compile(repo_root: &Path, state: &Path) -> CompileProfileBuildResult {
    let result = compile_profile_build(compile_request(repo_root, state)).unwrap();
    assert_eq!(result.diagnostics, Vec::new());
    result
}

fn compile_request(repo_root: &Path, state: &Path) -> CompileProfileBuildRequest {
    CompileProfileBuildRequest {
        repo_root: repo_root.to_path_buf(),
        user_state_dir: state.to_path_buf(),
        native_home_dir: Some(native_home_with_codex_auth(state)),
        profile: "github-researcher".to_string(),
        runtime: Some("codex".to_string()),
        env: BTreeMap::new(),
    }
}

fn native_home_with_codex_auth(root: &Path) -> PathBuf {
    let home = root.join("native-home");
    fs::create_dir_all(home.join(".codex")).unwrap();
    fs::write(home.join(".codex/auth.json"), br#"{"token":"test"}"#).unwrap();
    home
}

fn set_profile_runtimes(repo: &Path, runtimes: &str) {
    let path = repo.join("profiles/renamed-profile-dir/manifest.toml");
    let body = fs::read_to_string(&path).unwrap();
    let prefix = body.split("[runtimes.codex]").next().unwrap();
    fs::write(path, format!("{prefix}{runtimes}")).unwrap();
}

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
        r#"[runtimes.codex]
enabled = true
model = "gpt-5.4"
"#,
    );

    let build = compile(repo.path(), state.path()).build.unwrap();

    assert_eq!(
        fs::read_to_string(build.home_dir.join("config.toml")).unwrap(),
        "model = \"gpt-5.4\"\n\n[mcp_servers.linear]\ncommand = \"linear-mcp\"\n\n[mcp_servers.linear.env]\nLINEAR_API_KEY = \"required\"\n"
    );
}

#[test]
fn compile_warns_when_codex_auth_source_is_missing() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let request = compile_request(repo.path(), state.path());
    let native_home = request.native_home_dir.as_ref().unwrap();
    fs::remove_file(native_home.join(".codex/auth.json")).unwrap();

    let result = compile_profile_build(request).unwrap();

    assert!(result.build.is_some());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].severity, DiagnosticSeverity::Warning);
    assert_eq!(
        result.diagnostics[0].code,
        "runtime.credential-source-missing"
    );
    assert!(!result.build.unwrap().home_dir.join("auth.json").exists());
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
    let manifest = repo
        .path()
        .join("catalog/skills/renamed-skill-dir/manifest.toml");
    let updated = fs::read_to_string(&manifest).unwrap().replace(
        "[files]\nsource = \"SKILL.md\"",
        "[files]\nsource = \"SKILL.md\"\nreadme = \"README.md\"",
    );
    fs::write(manifest, updated).unwrap();

    let result = compile_profile_build(compile_request(repo.path(), state.path())).unwrap();

    assert!(result.build.is_none());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].code,
        "runtime.codex.file-mapping-unsupported"
    );
}

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

#[test]
fn compile_does_not_mutate_authored_catalog_files() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let before = file_snapshot(repo.path());

    let _ = compile(repo.path(), state.path());

    assert_eq!(file_snapshot(repo.path()), before);
}

#[test]
fn compile_warns_on_missing_required_env_and_still_writes_build() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    append_requires(
        repo.path(),
        "catalog/mcp/linear/manifest.toml",
        "env = [\"LINEAR_API_KEY\"]\n",
    );

    let result = compile_profile_build(compile_request(repo.path(), state.path())).unwrap();

    assert!(result.build.is_some());
    assert!(!result.has_error_diagnostics());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].severity, DiagnosticSeverity::Warning);
    assert_eq!(result.diagnostics[0].code, "profile.required-env-missing");
}

#[test]
fn compile_fails_on_missing_required_capability() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    append_requires(
        repo.path(),
        "catalog/mcp/linear/manifest.toml",
        "capabilities = [\"mcp:context-matters\"]\n",
    );

    let result = compile_profile_build(compile_request(repo.path(), state.path())).unwrap();

    assert!(result.build.is_none());
    assert!(result.has_error_diagnostics());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].code,
        "profile.required-capability-missing"
    );
}

#[test]
fn compile_fails_on_runtime_not_enabled_for_profile() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let mut request = compile_request(repo.path(), state.path());
    request.runtime = Some("claude".to_string());

    let result = compile_profile_build(request).unwrap();

    assert!(result.build.is_none());
    assert!(result.has_error_diagnostics());
    assert_eq!(
        result.diagnostics[0].code,
        "profile.build-plan.runtime-unavailable"
    );
}

#[test]
fn compile_fails_when_capability_does_not_support_requested_runtime() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    set_profile_runtimes(
        repo.path(),
        r#"[runtimes.codex]
enabled = true

[runtimes.claude]
enabled = true
"#,
    );
    let mut request = compile_request(repo.path(), state.path());
    request.runtime = Some("claude".to_string());

    let result = compile_profile_build(request).unwrap();

    assert!(result.build.is_none());
    assert!(result.has_error_diagnostics());
    assert_eq!(
        result.diagnostics[0].code,
        "profile.runtime.capability-unsupported"
    );
}

#[test]
fn compile_json_output_does_not_include_env_values() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    append_requires(
        repo.path(),
        "catalog/mcp/linear/manifest.toml",
        "env = [\"LINEAR_API_KEY\"]\n",
    );
    let mut request = compile_request(repo.path(), state.path());
    request.env = BTreeMap::from([(
        "LINEAR_API_KEY".to_string(),
        "secret-value-never-rendered".to_string(),
    )]);

    let result = compile_profile_build(request).unwrap();
    let encoded = serde_json::to_value(&result).unwrap();

    assert_eq!(encoded["profile"], "github-researcher");
    assert_eq!(encoded["build"]["runtime"], "codex");
    assert_eq!(encoded["diagnostics"], Value::Array(Vec::new()));
    assert!(!encoded.to_string().contains("secret-value-never-rendered"));
}

fn append_requires(repo: &Path, manifest: &str, body: &str) {
    let path = repo.join(manifest);
    let mut updated = fs::read_to_string(&path).unwrap();
    updated.push_str("\n[requires]\n");
    updated.push_str(body);
    fs::write(path, updated).unwrap();
}

fn file_snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut snapshot = BTreeMap::new();
    collect_files(root, root, &mut snapshot);
    snapshot
}

fn collect_files(root: &Path, current: &Path, snapshot: &mut BTreeMap<String, Vec<u8>>) {
    for entry in fs::read_dir(current).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, snapshot);
        } else {
            let relative = stable_path(path.strip_prefix(root).unwrap());
            snapshot.insert(relative, fs::read(path).unwrap());
        }
    }
}

fn stable_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

trait BuildPathAssertions {
    fn plan_home_path(&self) -> String;
}

impl BuildPathAssertions for agent_matters_capabilities::profiles::WrittenProfileBuild {
    fn plan_home_path(&self) -> String {
        PathBuf::from("builds")
            .join(&self.runtime)
            .join(&self.profile)
            .join(&self.build_id)
            .join("home")
            .to_string_lossy()
            .to_string()
    }
}
