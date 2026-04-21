use std::fs;
use std::path::Path;

use agent_matters_capabilities::profiles::{
    BuildProfilePlanRequest, ProfileBuildPlan, plan_profile_build,
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

pub(crate) fn build_plan_request(repo_root: &Path, state: &Path) -> BuildProfilePlanRequest {
    BuildProfilePlanRequest {
        repo_root: repo_root.to_path_buf(),
        user_state_dir: state.to_path_buf(),
        profile: "github-researcher".to_string(),
        runtime: Some("codex".to_string()),
    }
}

pub(crate) fn plan(repo_root: &Path, state: &Path) -> ProfileBuildPlan {
    let result = plan_profile_build(build_plan_request(repo_root, state)).unwrap();

    assert_eq!(result.diagnostics, Vec::new());
    result.plan.unwrap()
}

pub(crate) fn write(root: &Path, rel: &str, body: &str) {
    let full = root.join(rel);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(full, body).unwrap();
}

pub(crate) fn set_profile_runtimes(repo: &Path, runtimes: &str) {
    let path = repo.join("profiles/renamed-profile-dir/manifest.toml");
    let body = fs::read_to_string(&path).unwrap();
    let prefix = body.split("[runtimes.codex]").next().unwrap();
    fs::write(path, format!("{prefix}{runtimes}")).unwrap();
}
