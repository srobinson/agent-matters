//! Filesystem writer for immutable profile builds and stable runtime pointers.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use agent_matters_core::domain::{Diagnostic, DiagnosticLocation, DiagnosticSeverity};
use agent_matters_core::runtime::{
    BUILD_PLAN_FILE_NAME, RUNTIME_HOME_DIR_NAME, runtime_build_plan_file, runtime_pointer_target,
};
use serde::Serialize;

use super::ProfileBuildPlan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteProfileBuildRequest {
    pub user_state_dir: PathBuf,
    pub plan: ProfileBuildPlan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WriteProfileBuildResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<WrittenProfileBuild>,
    pub diagnostics: Vec<Diagnostic>,
}

impl WriteProfileBuildResult {
    pub fn has_error_diagnostics(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WrittenProfileBuild {
    pub profile: String,
    pub runtime: String,
    pub fingerprint: String,
    pub build_id: String,
    pub status: ProfileBuildWriteStatus,
    pub build_dir: PathBuf,
    pub home_dir: PathBuf,
    pub runtime_pointer: PathBuf,
    pub pointer_target: PathBuf,
    pub build_plan_path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProfileBuildWriteStatus {
    Created,
    Reused,
}

pub fn write_profile_build(request: WriteProfileBuildRequest) -> WriteProfileBuildResult {
    let paths = AbsoluteBuildPaths::new(&request.user_state_dir, &request.plan);
    let mut result = WriteProfileBuildResult {
        build: None,
        diagnostics: Vec::new(),
    };

    let status = match write_immutable_build(&paths, &request.plan) {
        Ok(status) => status,
        Err(source) => {
            result.diagnostics.push(write_diagnostic(
                "write immutable build",
                &paths.build_dir,
                &source,
            ));
            return result;
        }
    };

    if let Err(source) = update_runtime_pointer(&paths.runtime_pointer, &paths.pointer_target) {
        result.diagnostics.push(write_diagnostic(
            "update runtime pointer",
            &paths.runtime_pointer,
            &source,
        ));
        return result;
    }

    result.build = Some(WrittenProfileBuild {
        profile: request.plan.profile,
        runtime: request.plan.runtime,
        fingerprint: request.plan.fingerprint,
        build_id: request.plan.build_id,
        status,
        build_dir: paths.build_dir,
        home_dir: paths.home_dir,
        runtime_pointer: paths.runtime_pointer,
        pointer_target: paths.pointer_target,
        build_plan_path: paths.build_plan_path,
    });
    result
}

fn write_immutable_build(
    paths: &AbsoluteBuildPaths,
    plan: &ProfileBuildPlan,
) -> io::Result<ProfileBuildWriteStatus> {
    if paths.build_dir.exists() {
        validate_existing_build(paths)?;
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
    write_build_plan(&temp_dir.join(BUILD_PLAN_FILE_NAME), plan)?;

    match fs::rename(&temp_dir, &paths.build_dir) {
        Ok(()) => Ok(ProfileBuildWriteStatus::Created),
        Err(_source) if paths.build_dir.exists() => {
            remove_path_if_exists(&temp_dir)?;
            validate_existing_build(paths)?;
            Ok(ProfileBuildWriteStatus::Reused)
        }
        Err(source) => {
            remove_path_if_exists(&temp_dir)?;
            Err(source)
        }
    }
}

fn validate_existing_build(paths: &AbsoluteBuildPaths) -> io::Result<()> {
    if !paths.build_dir.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "build path exists but is not a directory",
        ));
    }
    if !paths.home_dir.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "build path exists without a runtime home directory",
        ));
    }
    if !paths.build_plan_path.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "build path exists without build plan metadata",
        ));
    }
    Ok(())
}

fn write_build_plan(path: &Path, plan: &ProfileBuildPlan) -> io::Result<()> {
    let mut encoded = serde_json::to_string_pretty(plan).map_err(io::Error::other)?;
    encoded.push('\n');
    fs::write(path, encoded)
}

fn update_runtime_pointer(pointer_path: &Path, pointer_target: &Path) -> io::Result<()> {
    let parent = pointer_path
        .parent()
        .ok_or_else(|| io::Error::other("runtime pointer has no parent"))?;
    fs::create_dir_all(parent)?;

    let temp_link = temp_sibling(pointer_path, "pointer");
    remove_path_if_exists(&temp_link)?;
    create_dir_symlink(pointer_target, &temp_link)?;

    match fs::rename(&temp_link, pointer_path) {
        Ok(()) => Ok(()),
        Err(source) => {
            remove_path_if_exists(&temp_link)?;
            Err(source)
        }
    }
}

fn temp_sibling(path: &Path, label: &str) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("runtime-home");
    path.with_file_name(format!(".{name}.{label}.tmp-{}", std::process::id()))
}

fn remove_path_if_exists(path: &Path) -> io::Result<()> {
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
fn create_dir_symlink(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_dir_symlink(target: &Path, link: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link)
}

fn write_diagnostic(action: &str, path: &Path, source: &io::Error) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "profile.build.write-failed",
        format!("failed to {action} `{}`: {source}", path.display()),
    )
    .with_location(DiagnosticLocation::manifest_path(path))
    .with_recovery_hint("check permissions and remove any non-directory path at that location")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AbsoluteBuildPaths {
    build_dir: PathBuf,
    home_dir: PathBuf,
    runtime_pointer: PathBuf,
    pointer_target: PathBuf,
    build_plan_path: PathBuf,
}

impl AbsoluteBuildPaths {
    fn new(user_state_dir: &Path, plan: &ProfileBuildPlan) -> Self {
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
