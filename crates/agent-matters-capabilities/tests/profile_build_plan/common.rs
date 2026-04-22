use std::fs;
use std::path::Path;

use agent_matters_capabilities::profiles::{
    BuildProfilePlanRequest, ProfileBuildPlan, plan_profile_build,
};
use tempfile::TempDir;

use crate::support::fixtures::valid_catalog_repo;
pub(crate) use crate::support::manifests::{ProfileRuntimeFixture, set_profile_runtimes};

pub(crate) fn valid_repo() -> TempDir {
    valid_catalog_repo()
}

pub(crate) const PROFILE_MANIFEST: &str = "profiles/renamed-profile-dir/manifest.toml";

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
