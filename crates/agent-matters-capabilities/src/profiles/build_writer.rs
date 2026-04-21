//! Filesystem writer for immutable profile builds and stable runtime pointers.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use agent_matters_core::domain::{Diagnostic, DiagnosticLocation, DiagnosticSeverity};
use agent_matters_core::runtime::{
    BUILD_PLAN_FILE_NAME, RUNTIME_HOME_DIR_NAME, RuntimeHomeFile, runtime_build_plan_file,
    runtime_pointer_target,
};
use serde::Serialize;

use super::{
    AssembleProfileInstructionsRequest, ProfileBuildPlan, RuntimeHomeRenderRequest,
    RuntimeHomeRenderResult, adapter_for_runtime, assemble_profile_instructions,
    unknown_runtime_adapter,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteProfileBuildRequest {
    pub repo_root: PathBuf,
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
    let assembled = assemble_profile_instructions(AssembleProfileInstructionsRequest {
        repo_root: &request.repo_root,
        profile: &request.plan.profile,
        fragments: &request.plan.instruction_fragments,
        output: &request.plan.instruction_output,
    });
    if has_error_diagnostics(&assembled.diagnostics) {
        result.diagnostics = assembled.diagnostics;
        return result;
    }
    let Some(instructions) = assembled.instructions else {
        result.diagnostics = assembled.diagnostics;
        return result;
    };
    let Some(adapter) = adapter_for_runtime(&request.plan.runtime) else {
        result
            .diagnostics
            .push(unknown_runtime_adapter(&request.plan.runtime));
        return result;
    };
    let home = adapter.render_home(RuntimeHomeRenderRequest {
        plan: &request.plan,
        instructions: &instructions,
    });
    result.diagnostics.extend(home.diagnostics.clone());
    if has_error_diagnostics(&result.diagnostics) {
        return result;
    }

    let status = match write_immutable_build(&paths, &request.plan, &home) {
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
    home: &RuntimeHomeRenderResult,
) -> io::Result<ProfileBuildWriteStatus> {
    if paths.build_dir.exists() {
        validate_existing_build(paths, plan, home)?;
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
    write_build_plan(&temp_dir.join(BUILD_PLAN_FILE_NAME), plan)?;

    match fs::rename(&temp_dir, &paths.build_dir) {
        Ok(()) => Ok(ProfileBuildWriteStatus::Created),
        Err(_source) if paths.build_dir.exists() => {
            remove_path_if_exists(&temp_dir)?;
            validate_existing_build(paths, plan, home)?;
            Ok(ProfileBuildWriteStatus::Reused)
        }
        Err(source) => {
            remove_path_if_exists(&temp_dir)?;
            Err(source)
        }
    }
}

fn validate_existing_build(
    paths: &AbsoluteBuildPaths,
    plan: &ProfileBuildPlan,
    home: &RuntimeHomeRenderResult,
) -> io::Result<()> {
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
    validate_existing_runtime_home(paths, &home.files)?;
    validate_existing_build_plan(paths, plan)?;
    Ok(())
}

fn validate_existing_build_plan(
    paths: &AbsoluteBuildPaths,
    plan: &ProfileBuildPlan,
) -> io::Result<()> {
    let encoded = fs::read_to_string(&paths.build_plan_path)?;
    let existing: serde_json::Value = serde_json::from_str(&encoded).map_err(|source| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("existing build plan metadata is invalid: {source}"),
        )
    })?;
    let expected = serde_json::to_value(plan).map_err(io::Error::other)?;
    if existing != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "existing build plan metadata does not match requested plan",
        ));
    }
    Ok(())
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

fn validate_existing_runtime_home(
    paths: &AbsoluteBuildPaths,
    files: &[RuntimeHomeFile],
) -> io::Result<()> {
    for file in files {
        let path = runtime_home_file_path(&paths.home_dir, &file.relative_path)?;
        let existing = fs::read(&path).map_err(|source| {
            if source.kind() == io::ErrorKind::NotFound {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "build path exists without runtime home file `{}`",
                        file.relative_path.display()
                    ),
                )
            } else {
                source
            }
        })?;
        if existing != file.contents {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "existing runtime home file `{}` does not match requested plan",
                    file.relative_path.display()
                ),
            ));
        }
    }
    Ok(())
}

fn runtime_home_file_path(home_dir: &Path, relative_path: &Path) -> io::Result<PathBuf> {
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

fn has_error_diagnostics(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
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
