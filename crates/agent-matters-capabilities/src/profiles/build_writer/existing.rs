use std::fs;
use std::io;

use agent_matters_core::runtime::RuntimeHomeFile;

use super::super::{ProfileBuildPlan, RuntimeAdapter, RuntimeHomeRenderResult};
use super::paths::{AbsoluteBuildPaths, runtime_home_file_path};

pub(super) fn validate_existing_build(
    paths: &AbsoluteBuildPaths,
    plan: &ProfileBuildPlan,
    adapter: &dyn RuntimeAdapter,
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
    validate_existing_runtime_home(paths, adapter, &home.files)?;
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

fn validate_existing_runtime_home(
    paths: &AbsoluteBuildPaths,
    adapter: &dyn RuntimeAdapter,
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
        if !adapter.existing_home_file_matches(file, &existing) {
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
