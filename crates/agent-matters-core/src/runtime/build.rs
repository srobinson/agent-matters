//! Generated runtime home path conventions.

use std::path::PathBuf;

/// Schema version for serializable build plans.
pub const BUILD_PLAN_SCHEMA_VERSION: u16 = 1;

/// Generated machine readable plan written beside an immutable build.
pub const BUILD_PLAN_FILE_NAME: &str = "build-plan.json";

/// User state subdirectory containing immutable runtime builds.
pub const BUILDS_DIR_NAME: &str = "builds";

/// User state subdirectory containing stable runtime pointers.
pub const RUNTIMES_DIR_NAME: &str = "runtimes";

/// Directory inside a build that is directly usable as the runtime home.
pub const RUNTIME_HOME_DIR_NAME: &str = "home";

/// Runtime instruction file assembled from profile instruction fragments.
pub const RUNTIME_INSTRUCTIONS_FILE_NAME: &str = "AGENTS.md";

pub fn runtime_build_dir(runtime: &str, profile: &str, build_id: &str) -> PathBuf {
    PathBuf::from(BUILDS_DIR_NAME)
        .join(runtime)
        .join(profile)
        .join(build_id)
}

pub fn runtime_home_dir(runtime: &str, profile: &str, build_id: &str) -> PathBuf {
    runtime_build_dir(runtime, profile, build_id).join(RUNTIME_HOME_DIR_NAME)
}

pub fn runtime_build_plan_file(runtime: &str, profile: &str, build_id: &str) -> PathBuf {
    runtime_build_dir(runtime, profile, build_id).join(BUILD_PLAN_FILE_NAME)
}

pub fn runtime_pointer_dir(profile: &str, runtime: &str) -> PathBuf {
    PathBuf::from(RUNTIMES_DIR_NAME).join(profile).join(runtime)
}

pub fn runtime_pointer_target(runtime: &str, profile: &str, build_id: &str) -> PathBuf {
    PathBuf::from("..")
        .join("..")
        .join(runtime_home_dir(runtime, profile, build_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_paths_match_generated_home_contract() {
        assert_eq!(
            runtime_build_dir("codex", "github-researcher", "3f8a91c2"),
            PathBuf::from("builds/codex/github-researcher/3f8a91c2")
        );
        assert_eq!(
            runtime_home_dir("codex", "github-researcher", "3f8a91c2"),
            PathBuf::from("builds/codex/github-researcher/3f8a91c2/home")
        );
        assert_eq!(
            runtime_build_plan_file("codex", "github-researcher", "3f8a91c2"),
            PathBuf::from("builds/codex/github-researcher/3f8a91c2/build-plan.json")
        );
        assert_eq!(
            runtime_pointer_dir("github-researcher", "codex"),
            PathBuf::from("runtimes/github-researcher/codex")
        );
        assert_eq!(
            runtime_pointer_target("codex", "github-researcher", "3f8a91c2"),
            PathBuf::from("../../builds/codex/github-researcher/3f8a91c2/home")
        );
    }
}
