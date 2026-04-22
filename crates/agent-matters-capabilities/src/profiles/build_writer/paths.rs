use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use agent_matters_core::runtime::{runtime_build_plan_file, runtime_pointer_target};

use super::super::ProfileBuildPlan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AbsoluteBuildPaths {
    pub(super) build_dir: PathBuf,
    pub(super) home_dir: PathBuf,
    pub(super) runtime_pointer: PathBuf,
    pub(super) pointer_target: PathBuf,
    pub(super) build_plan_path: PathBuf,
}

impl AbsoluteBuildPaths {
    pub(super) fn new(user_state_dir: &Path, plan: &ProfileBuildPlan) -> Self {
        Self {
            build_dir: user_state_dir.join(&plan.paths.build_dir),
            home_dir: user_state_dir.join(&plan.paths.home_dir),
            runtime_pointer: user_state_dir.join(&plan.paths.runtime_pointer),
            pointer_target: runtime_pointer_target(&plan.runtime, &plan.profile, &plan.build_id),
            build_plan_path: user_state_dir.join(runtime_build_plan_file(
                &plan.runtime,
                &plan.profile,
                &plan.build_id,
            )),
        }
    }
}

pub(super) fn runtime_home_file_path(home_dir: &Path, relative_path: &Path) -> io::Result<PathBuf> {
    if relative_path.is_absolute()
        || relative_path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "runtime home file path `{}` must be relative",
                relative_path.display()
            ),
        ));
    }

    Ok(home_dir.join(relative_path))
}

pub(super) fn temp_sibling(path: &Path, label: &str) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("runtime-home");
    path.with_file_name(format!(".{name}.{label}.tmp-{}", std::process::id()))
}

pub(super) fn remove_path_if_exists(path: &Path) -> io::Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source) => return Err(source),
    };

    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

#[cfg(unix)]
pub(super) fn create_dir_symlink(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
pub(super) fn create_dir_symlink(target: &Path, link: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link)
}
