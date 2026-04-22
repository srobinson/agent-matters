use std::fs;
use std::path::PathBuf;

use agent_matters_capabilities::profiles::{ProfileBuildWriteStatus, compile_profile_build};
use agent_matters_core::domain::DiagnosticSeverity;
use tempfile::TempDir;

use crate::common::{compile_request, valid_repo};

#[test]
fn compile_warns_when_codex_auth_source_is_missing() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let request = compile_request(repo.path(), state.path());
    let native_home = request.native_home_dir.as_ref().unwrap();
    fs::remove_file(native_home.join(".codex/auth.json")).unwrap();

    let result = compile_profile_build(request).unwrap();

    assert!(result.build.is_some());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].severity, DiagnosticSeverity::Warning);
    assert_eq!(
        result.diagnostics[0].code,
        "runtime.credential-source-missing"
    );
    assert!(matches!(
        fs::symlink_metadata(result.build.unwrap().home_dir.join("auth.json")),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound
    ));
}

#[test]
fn compile_removes_stale_codex_auth_symlink_when_source_is_missing_on_reuse() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let request = compile_request(repo.path(), state.path());
    let native_home = request.native_home_dir.clone().unwrap();
    let first = compile_profile_build(request.clone())
        .unwrap()
        .build
        .unwrap();
    assert_eq!(
        fs::read_link(first.home_dir.join("auth.json")).unwrap(),
        native_home.join(".codex/auth.json")
    );
    fs::remove_file(native_home.join(".codex/auth.json")).unwrap();

    let result = compile_profile_build(request).unwrap();

    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].code,
        "runtime.credential-source-missing"
    );
    let build = result.build.unwrap();
    assert_eq!(build.status, ProfileBuildWriteStatus::Reused);
    assert!(matches!(
        fs::symlink_metadata(build.home_dir.join("auth.json")),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound
    ));
}

#[test]
fn compile_removes_stale_codex_auth_symlink_when_native_home_is_missing_on_reuse() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let request = compile_request(repo.path(), state.path());
    let native_home = request.native_home_dir.clone().unwrap();
    let first = compile_profile_build(request.clone())
        .unwrap()
        .build
        .unwrap();
    assert_eq!(
        fs::read_link(first.home_dir.join("auth.json")).unwrap(),
        native_home.join(".codex/auth.json")
    );
    let mut missing_home_request = request;
    missing_home_request.native_home_dir = None;

    let result = compile_profile_build(missing_home_request).unwrap();

    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].code,
        "runtime.credential-home-missing"
    );
    let build = result.build.unwrap();
    assert_eq!(build.status, ProfileBuildWriteStatus::Reused);
    assert_eq!(build.credential_symlinks.len(), 1);
    assert_eq!(build.credential_symlinks[0].source_path, None);
    assert_eq!(
        build.credential_symlinks[0].target_path,
        PathBuf::from("auth.json")
    );
    assert!(matches!(
        fs::symlink_metadata(build.home_dir.join("auth.json")),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound
    ));
}

#[test]
fn compile_fingerprint_excludes_codex_auth_contents() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let request = compile_request(repo.path(), state.path());
    let native_home = request.native_home_dir.clone().unwrap();
    let first = compile_profile_build(request.clone())
        .unwrap()
        .build
        .unwrap();

    fs::write(
        native_home.join(".codex/auth.json"),
        br#"{"token":"changed-without-new-build"}"#,
    )
    .unwrap();
    let result = compile_profile_build(request).unwrap();

    assert_eq!(result.diagnostics, Vec::new());
    let second = result.build.unwrap();
    assert_eq!(first.fingerprint, second.fingerprint);
    assert_eq!(first.build_id, second.build_id);
    assert_eq!(second.status, ProfileBuildWriteStatus::Reused);
    assert_eq!(
        fs::read_to_string(second.home_dir.join("auth.json")).unwrap(),
        r#"{"token":"changed-without-new-build"}"#
    );
}
