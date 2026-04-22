//! Runtime credential symlink planning and materialization.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use agent_matters_core::domain::{Diagnostic, DiagnosticLocation, DiagnosticSeverity};
use agent_matters_core::runtime::{CredentialSymlinkAllowlistEntry, RuntimeCredentialSymlink};

use super::RuntimeAdapter;

pub(super) type CredentialSymlink = RuntimeCredentialSymlink;

pub(super) fn credential_symlinks_for_adapter(
    adapter: &'static dyn RuntimeAdapter,
    native_home_dir: Option<&Path>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<CredentialSymlink> {
    let allowlist = adapter.credential_symlink_allowlist();
    if allowlist.is_empty() {
        return Vec::new();
    }
    let Some(native_home_dir) = native_home_dir else {
        diagnostics.push(credential_home_missing(adapter.id()));
        return missing_credential_symlinks(&allowlist);
    };
    let Some(source_dir) = adapter.credential_source_dir(native_home_dir) else {
        return Vec::new();
    };

    allowlist
        .iter()
        .map(|entry| credential_symlink(adapter.id(), &source_dir, entry, diagnostics))
        .collect()
}

fn missing_credential_symlinks(
    allowlist: &[CredentialSymlinkAllowlistEntry],
) -> Vec<CredentialSymlink> {
    allowlist
        .iter()
        .map(|entry| CredentialSymlink::new(None, &entry.target_path))
        .collect()
}

pub(super) fn write_credential_symlinks(
    home_dir: &Path,
    credential_symlinks: &[CredentialSymlink],
) -> io::Result<()> {
    for symlink in credential_symlinks {
        let link = runtime_home_link_path(home_dir, &symlink.target_path)?;
        if let Some(source_path) = &symlink.source_path {
            if let Some(parent) = link.parent() {
                fs::create_dir_all(parent)?;
            }
            upsert_file_symlink(source_path, &link)?;
        } else {
            remove_stale_file_symlink(&link)?;
        }
    }
    Ok(())
}

fn credential_symlink(
    runtime: &str,
    source_dir: &Path,
    entry: &CredentialSymlinkAllowlistEntry,
    diagnostics: &mut Vec<Diagnostic>,
) -> CredentialSymlink {
    let source_path = source_dir.join(&entry.source_name);
    if !source_path.is_file() {
        diagnostics.push(credential_source_missing(runtime, &source_path));
        return CredentialSymlink::new(None, &entry.target_path);
    }

    CredentialSymlink::new(Some(source_path), &entry.target_path)
}

fn runtime_home_link_path(home_dir: &Path, relative_path: &Path) -> io::Result<PathBuf> {
    if relative_path.is_absolute()
        || relative_path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "runtime credential link path `{}` must be relative",
                relative_path.display()
            ),
        ));
    }

    Ok(home_dir.join(relative_path))
}

fn upsert_file_symlink(target: &Path, link: &Path) -> io::Result<()> {
    match fs::symlink_metadata(link) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            if fs::read_link(link)? == target {
                return Ok(());
            }
            fs::remove_file(link)?;
        }
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "credential link path `{}` exists and is not a symlink",
                    link.display()
                ),
            ));
        }
        Err(source) if source.kind() == io::ErrorKind::NotFound => {}
        Err(source) => return Err(source),
    }

    create_file_symlink(target, link)
}

fn remove_stale_file_symlink(link: &Path) -> io::Result<()> {
    match fs::symlink_metadata(link) {
        Ok(metadata) if metadata.file_type().is_symlink() => fs::remove_file(link),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "credential link path `{}` exists and is not a symlink",
                link.display()
            ),
        )),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(source),
    }
}

#[cfg(unix)]
fn create_file_symlink(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_file_symlink(target: &Path, link: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

fn credential_home_missing(runtime: &str) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Warning,
        "runtime.credential-home-missing",
        format!("cannot locate native home for `{runtime}` credential symlinks"),
    )
    .with_recovery_hint("set HOME or pass a native home directory before launching the runtime")
}

fn credential_source_missing(runtime: &str, source_path: &Path) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Warning,
        "runtime.credential-source-missing",
        format!(
            "`{runtime}` credential source `{}` does not exist",
            source_path.display()
        ),
    )
    .with_location(DiagnosticLocation::manifest_path(source_path))
    .with_recovery_hint("authenticate with the native runtime before using this generated home")
}
