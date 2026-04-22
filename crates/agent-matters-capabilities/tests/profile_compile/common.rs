use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use agent_matters_capabilities::profiles::{
    CompileProfileBuildRequest, CompileProfileBuildResult, compile_profile_build,
};
use tempfile::TempDir;

use crate::support::fixtures::valid_catalog_repo;
pub(crate) use crate::support::manifests::{
    ProfileRuntimeFixture, add_capability_file_mapping, add_required_capability, add_required_env,
    set_profile_runtimes,
};
pub(crate) use crate::support::native_home::native_home_with_codex_auth;

pub(crate) fn valid_repo() -> TempDir {
    valid_catalog_repo()
}

pub(crate) const PROFILE_MANIFEST: &str = "profiles/renamed-profile-dir/manifest.toml";

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
