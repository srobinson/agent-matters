use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use agent_matters_core::domain::{Diagnostic, DiagnosticSeverity};
use agent_matters_core::runtime::{BUILDS_DIR_NAME, RUNTIME_HOME_DIR_NAME, RUNTIMES_DIR_NAME};

use crate::doctor::DoctorGeneratedStateSummary;

pub(super) fn inspect_generated_state(
    user_state_dir: &Path,
    diagnostics: &mut Vec<Diagnostic>,
) -> DoctorGeneratedStateSummary {
    let writable = state_root_is_writable(user_state_dir, diagnostics);
    let runtime_pointer_count = if user_state_dir.is_dir() {
        inspect_runtime_pointers(user_state_dir, diagnostics)
    } else {
        0
    };

    DoctorGeneratedStateSummary {
        path: user_state_dir.to_path_buf(),
        writable,
        runtime_pointer_count,
    }
}

fn state_root_is_writable(path: &Path, diagnostics: &mut Vec<Diagnostic>) -> bool {
    match fs::metadata(path) {
        Ok(metadata) if !metadata.is_dir() => {
            diagnostics.push(state_root_not_directory(path));
            false
        }
        Ok(metadata) => {
            let writable = directory_is_writable(&metadata);
            if !writable {
                diagnostics.push(state_root_not_writable(path, path));
            }
            writable
        }
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            missing_state_root_parent_is_writable(path, diagnostics)
        }
        Err(source) => {
            diagnostics.push(state_root_read_failed(path, &source));
            false
        }
    }
}

fn missing_state_root_parent_is_writable(path: &Path, diagnostics: &mut Vec<Diagnostic>) -> bool {
    for ancestor in path.ancestors().skip(1) {
        if ancestor.as_os_str().is_empty() {
            continue;
        }
        match fs::metadata(ancestor) {
            Ok(metadata) if metadata.is_dir() => {
                let writable = directory_is_writable(&metadata);
                if !writable {
                    diagnostics.push(state_root_not_writable(path, ancestor));
                }
                return writable;
            }
            Ok(_) => continue,
            Err(source) if source.kind() == io::ErrorKind::NotFound => continue,
            Err(source) => {
                diagnostics.push(state_root_read_failed(ancestor, &source));
                return false;
            }
        }
    }

    diagnostics.push(state_root_not_writable(path, path));
    false
}

fn directory_is_writable(metadata: &fs::Metadata) -> bool {
    !metadata.permissions().readonly()
}

fn inspect_runtime_pointers(user_state_dir: &Path, diagnostics: &mut Vec<Diagnostic>) -> usize {
    let runtimes_root = user_state_dir.join(RUNTIMES_DIR_NAME);
    let profiles = match fs::read_dir(&runtimes_root) {
        Ok(entries) => entries,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return 0,
        Err(source) => {
            diagnostics.push(runtime_pointer_read_failed(&runtimes_root, &source));
            return 0;
        }
    };
    let mut count = 0;

    for profile in profiles {
        let Ok(profile) = profile else {
            diagnostics.push(runtime_pointer_read_failed(
                &runtimes_root,
                &io::Error::other("failed to read runtime pointer profile entry"),
            ));
            continue;
        };
        let profile_path = profile.path();
        if !profile_path.is_dir() {
            continue;
        }
        let profile_id = profile.file_name().to_string_lossy().into_owned();
        count += inspect_profile_runtime_pointers(
            user_state_dir,
            &profile_id,
            &profile_path,
            diagnostics,
        );
    }

    count
}

fn inspect_profile_runtime_pointers(
    user_state_dir: &Path,
    profile: &str,
    profile_path: &Path,
    diagnostics: &mut Vec<Diagnostic>,
) -> usize {
    let runtimes = match fs::read_dir(profile_path) {
        Ok(entries) => entries,
        Err(source) => {
            diagnostics.push(runtime_pointer_read_failed(profile_path, &source));
            return 0;
        }
    };
    let mut count = 0;

    for runtime in runtimes {
        let Ok(runtime) = runtime else {
            diagnostics.push(runtime_pointer_read_failed(
                profile_path,
                &io::Error::other("failed to read runtime pointer entry"),
            ));
            continue;
        };
        let runtime_path = runtime.path();
        let runtime_id = runtime.file_name().to_string_lossy().into_owned();
        count += 1;
        validate_runtime_pointer(
            user_state_dir,
            profile,
            &runtime_id,
            &runtime_path,
            diagnostics,
        );
    }

    count
}

fn validate_runtime_pointer(
    user_state_dir: &Path,
    profile: &str,
    runtime: &str,
    path: &Path,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(source) => {
            diagnostics.push(runtime_pointer_read_failed(path, &source));
            return;
        }
    };
    if !metadata.file_type().is_symlink() {
        diagnostics.push(runtime_pointer_not_symlink(profile, runtime, path));
        return;
    }

    let target = match fs::read_link(path) {
        Ok(target) => target,
        Err(source) => {
            diagnostics.push(runtime_pointer_read_failed(path, &source));
            return;
        }
    };
    let resolved = resolve_pointer_target(path, &target);
    if !resolved.is_dir() {
        diagnostics.push(runtime_pointer_target_invalid(
            profile, runtime, path, &target, &resolved,
        ));
        return;
    }
    if !runtime_pointer_target_matches_contract(user_state_dir, profile, runtime, &resolved) {
        diagnostics.push(runtime_pointer_target_unexpected(
            profile,
            runtime,
            path,
            &target,
            &resolved,
            user_state_dir,
        ));
    }
}

fn resolve_pointer_target(pointer_path: &Path, target: &Path) -> PathBuf {
    if target.is_absolute() {
        return target.to_path_buf();
    }
    pointer_path
        .parent()
        .map_or_else(|| target.to_path_buf(), |parent| parent.join(target))
}

fn runtime_pointer_target_matches_contract(
    user_state_dir: &Path,
    profile: &str,
    runtime: &str,
    resolved: &Path,
) -> bool {
    let Ok(resolved) = fs::canonicalize(resolved) else {
        return false;
    };
    let expected_build_root = user_state_dir
        .join(BUILDS_DIR_NAME)
        .join(runtime)
        .join(profile);
    let Ok(expected_build_root) = fs::canonicalize(expected_build_root) else {
        return false;
    };
    let Ok(relative) = resolved.strip_prefix(expected_build_root) else {
        return false;
    };

    let mut components = relative.components();
    let Some(Component::Normal(_build_id)) = components.next() else {
        return false;
    };
    let Some(Component::Normal(home)) = components.next() else {
        return false;
    };
    home == OsStr::new(RUNTIME_HOME_DIR_NAME) && components.next().is_none()
}

fn state_root_not_directory(path: &Path) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "runtime.state-root-not-directory",
        format!(
            "generated state root `{}` is not a directory",
            path.display()
        ),
    )
    .with_recovery_hint("choose a directory path for AGENT_MATTERS_STATE_DIR")
}

fn state_root_not_writable(path: &Path, checked: &Path) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "runtime.state-root-not-writable",
        format!(
            "generated state root `{}` cannot be written because `{}` is not writable",
            path.display(),
            checked.display()
        ),
    )
    .with_recovery_hint("fix directory permissions before compiling runtime homes")
}

fn state_root_read_failed(path: &Path, source: &io::Error) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "runtime.state-root-read-failed",
        format!(
            "failed to inspect generated state path `{}`: {source}",
            path.display()
        ),
    )
    .with_recovery_hint("fix directory permissions before compiling runtime homes")
}

fn runtime_pointer_read_failed(path: &Path, source: &io::Error) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "runtime.pointer-read-failed",
        format!(
            "failed to inspect runtime pointer `{}`: {source}",
            path.display()
        ),
    )
    .with_recovery_hint("delete the broken runtime pointer or recompile the profile")
}

fn runtime_pointer_not_symlink(profile: &str, runtime: &str, path: &Path) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "runtime.pointer-not-symlink",
        format!(
            "runtime pointer for profile `{profile}` and runtime `{runtime}` at `{}` is not a symlink",
            path.display()
        ),
    )
    .with_recovery_hint("delete the invalid pointer and rerun `agent-matters profiles compile`")
}

fn runtime_pointer_target_invalid(
    profile: &str,
    runtime: &str,
    path: &Path,
    target: &Path,
    resolved: &Path,
) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "runtime.pointer-target-invalid",
        format!(
            "runtime pointer for profile `{profile}` and runtime `{runtime}` at `{}` points to `{}`, resolved as missing directory `{}`",
            path.display(),
            target.display(),
            resolved.display()
        ),
    )
    .with_recovery_hint("delete the broken pointer or recompile the profile")
}

fn runtime_pointer_target_unexpected(
    profile: &str,
    runtime: &str,
    path: &Path,
    target: &Path,
    resolved: &Path,
    user_state_dir: &Path,
) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "runtime.pointer-target-invalid",
        format!(
            "runtime pointer for profile `{profile}` and runtime `{runtime}` at `{}` points to `{}`, resolved as `{}`, but expected a generated home under `{}`",
            path.display(),
            target.display(),
            resolved.display(),
            user_state_dir
                .join(BUILDS_DIR_NAME)
                .join(runtime)
                .join(profile)
                .display()
        ),
    )
    .with_recovery_hint("delete the invalid pointer or recompile the profile")
}
