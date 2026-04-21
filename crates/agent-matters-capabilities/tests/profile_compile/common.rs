use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use agent_matters_capabilities::profiles::{
    CompileProfileBuildRequest, CompileProfileBuildResult, compile_profile_build,
};
use tempfile::TempDir;

use crate::support::fixture_path;

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

pub(crate) fn valid_repo() -> TempDir {
    let tmp = TempDir::new().unwrap();
    copy_dir(&fixture_path("catalogs/valid"), tmp.path());
    tmp
}

pub(crate) fn compile(repo_root: &Path, state: &Path) -> CompileProfileBuildResult {
    let result = compile_profile_build(compile_request(repo_root, state)).unwrap();
    assert_eq!(result.diagnostics, Vec::new());
    result
}

pub(crate) fn compile_request(repo_root: &Path, state: &Path) -> CompileProfileBuildRequest {
    CompileProfileBuildRequest {
        repo_root: repo_root.to_path_buf(),
        user_state_dir: state.to_path_buf(),
        native_home_dir: Some(native_home_with_codex_auth(state)),
        profile: "github-researcher".to_string(),
        runtime: Some("codex".to_string()),
        env: BTreeMap::new(),
    }
}

pub(crate) fn native_home_with_codex_auth(root: &Path) -> PathBuf {
    let home = root.join("native-home");
    fs::create_dir_all(home.join(".codex")).unwrap();
    fs::write(home.join(".codex/auth.json"), br#"{"token":"test"}"#).unwrap();
    home
}

pub(crate) fn set_profile_runtimes(repo: &Path, runtimes: &str) {
    let path = repo.join("profiles/renamed-profile-dir/manifest.toml");
    let body = fs::read_to_string(&path).unwrap();
    let prefix = body.split("[runtimes.codex]").next().unwrap();
    fs::write(path, format!("{prefix}{runtimes}")).unwrap();
}

pub(crate) fn append_requires(repo: &Path, manifest: &str, body: &str) {
    let path = repo.join(manifest);
    let mut updated = fs::read_to_string(&path).unwrap();
    updated.push_str("\n[requires]\n");
    updated.push_str(body);
    fs::write(path, updated).unwrap();
}

pub(crate) fn file_snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
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

pub(crate) trait BuildPathAssertions {
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
