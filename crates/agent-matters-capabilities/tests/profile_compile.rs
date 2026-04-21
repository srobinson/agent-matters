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
    let result = compile_profile_build(CompileProfileBuildRequest {
        repo_root: repo_root.to_path_buf(),
        user_state_dir: state.to_path_buf(),
        profile: "github-researcher".to_string(),
        runtime: Some("codex".to_string()),
    })
    .unwrap();
    assert_eq!(result.diagnostics, Vec::new());
    result
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

    let original = fs::metadata(state.path()).unwrap().permissions();
    let mut read_only = original.clone();
    read_only.set_mode(0o555);
    fs::set_permissions(state.path(), read_only).unwrap();
    let result = compile_profile_build(CompileProfileBuildRequest {
        repo_root: repo.path().to_path_buf(),
        user_state_dir: state.path().to_path_buf(),
        profile: "github-researcher".to_string(),
        runtime: Some("codex".to_string()),
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
