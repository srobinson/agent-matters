use std::fs;
use std::io;
use std::path::Path;

use agent_matters_core::runtime::{BUILD_PLAN_FILE_NAME, RUNTIME_HOME_DIR_NAME, RuntimeHomeFile};

use super::super::{
    ProfileBuildPlan, RuntimeAdapter, RuntimeHomeRenderResult,
    credential_symlinks::{CredentialSymlink, write_credential_symlinks},
};
use super::ProfileBuildWriteStatus;
use super::existing::validate_existing_build;
use super::paths::{
    AbsoluteBuildPaths, remove_path_if_exists, runtime_home_file_path, temp_sibling,
};

pub(super) fn write_immutable_build(
    paths: &AbsoluteBuildPaths,
    plan: &ProfileBuildPlan,
    adapter: &dyn RuntimeAdapter,
    home: &RuntimeHomeRenderResult,
    credential_symlinks: &[CredentialSymlink],
) -> io::Result<ProfileBuildWriteStatus> {
    if paths.build_dir.exists() {
        validate_existing_build(paths, plan, adapter, home)?;
        write_credential_symlinks(&paths.home_dir, credential_symlinks)?;
        return Ok(ProfileBuildWriteStatus::Reused);
    }

    let parent = paths
        .build_dir
        .parent()
        .ok_or_else(|| io::Error::other("build directory has no parent"))?;
    fs::create_dir_all(parent)?;

    let temp_dir = temp_sibling(&paths.build_dir, "build");
    remove_path_if_exists(&temp_dir)?;
    fs::create_dir_all(temp_dir.join(RUNTIME_HOME_DIR_NAME))?;
    write_runtime_home_files(&temp_dir.join(RUNTIME_HOME_DIR_NAME), &home.files)?;
    write_credential_symlinks(&temp_dir.join(RUNTIME_HOME_DIR_NAME), credential_symlinks)?;
    write_build_plan(&temp_dir.join(BUILD_PLAN_FILE_NAME), plan)?;

    match fs::rename(&temp_dir, &paths.build_dir) {
        Ok(()) => Ok(ProfileBuildWriteStatus::Created),
        Err(_source) if paths.build_dir.exists() => {
            remove_path_if_exists(&temp_dir)?;
            validate_existing_build(paths, plan, adapter, home)?;
            write_credential_symlinks(&paths.home_dir, credential_symlinks)?;
            Ok(ProfileBuildWriteStatus::Reused)
        }
        Err(source) => {
            remove_path_if_exists(&temp_dir)?;
            Err(source)
        }
    }
}

fn write_build_plan(path: &Path, plan: &ProfileBuildPlan) -> io::Result<()> {
    let mut encoded = serde_json::to_string_pretty(plan).map_err(io::Error::other)?;
    encoded.push('\n');
    fs::write(path, encoded)
}

fn write_runtime_home_files(home_dir: &Path, files: &[RuntimeHomeFile]) -> io::Result<()> {
    for file in files {
        let path = runtime_home_file_path(home_dir, &file.relative_path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, &file.contents)?;
    }
    Ok(())
}
