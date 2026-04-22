mod support;

use std::fs;
use std::path::PathBuf;

use agent_matters_capabilities::doctor::{DoctorIndexStatus, DoctorRequest, run_doctor};
use agent_matters_core::domain::{Diagnostic, DiagnosticSeverity};
use tempfile::TempDir;

use support::fixtures::valid_catalog_repo;
use support::manifests::{ProfileRuntimeFixture, set_profile_runtimes};
use support::native_home::native_home_with_codex_auth;

const PROFILE_MANIFEST: &str = "profiles/renamed-profile-dir/manifest.toml";

fn valid_repo() -> TempDir {
    valid_catalog_repo()
}

fn doctor_request(repo: &TempDir, state: &TempDir) -> DoctorRequest {
    DoctorRequest {
        repo_root: repo.path().to_path_buf(),
        user_state_dir: state.path().to_path_buf(),
        native_home_dir: None,
    }
}

fn doctor_request_with_native_home(
    repo: &TempDir,
    state: &TempDir,
    native_home: PathBuf,
) -> DoctorRequest {
    DoctorRequest {
        repo_root: repo.path().to_path_buf(),
        user_state_dir: state.path().to_path_buf(),
        native_home_dir: Some(native_home),
    }
}

fn diagnostic_with_code<'a>(diagnostics: &'a [Diagnostic], code: &str) -> &'a Diagnostic {
    diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == code)
        .unwrap_or_else(|| panic!("missing diagnostic {code}"))
}

#[test]
fn missing_codex_auth_is_warning() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let native_home = state.path().join("native-home");
    fs::create_dir_all(native_home.join(".codex")).unwrap();

    let result = run_doctor(doctor_request_with_native_home(&repo, &state, native_home)).unwrap();
    let encoded = serde_json::to_value(&result).unwrap();

    let missing = diagnostic_with_code(&result.diagnostics, "runtime.credential-source-missing");
    assert_eq!(missing.severity, DiagnosticSeverity::Warning);
    assert!(missing.message.contains("codex"));
    assert!(missing.message.contains("auth.json"));
    assert!(
        missing
            .recovery_hint
            .as_deref()
            .unwrap()
            .contains("authenticate")
    );
    assert_eq!(encoded["runtimes"][0]["id"], "codex");
    assert_eq!(
        encoded["diagnostics"][0]["code"],
        "runtime.credential-source-missing"
    );
    assert!(!result.has_error_diagnostics());
}

#[test]
fn missing_claude_credentials_is_warning() {
    let repo = valid_repo();
    set_profile_runtimes(
        repo.path(),
        PROFILE_MANIFEST,
        None,
        &[
            ProfileRuntimeFixture::enabled("codex"),
            ProfileRuntimeFixture::enabled("claude"),
        ],
    );
    let state = TempDir::new().unwrap();
    let native_home = native_home_with_codex_auth(state.path());
    fs::create_dir_all(native_home.join(".claude")).unwrap();

    let result = run_doctor(doctor_request_with_native_home(&repo, &state, native_home)).unwrap();

    let missing = result
        .diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic.code == "runtime.credential-source-missing"
                && diagnostic.message.contains("claude")
        })
        .expect("missing claude credential diagnostic");
    assert_eq!(missing.severity, DiagnosticSeverity::Warning);
    assert!(missing.message.contains(".credentials.json"));
    assert!(!result.has_error_diagnostics());
}

#[cfg(unix)]
#[test]
fn broken_runtime_pointer_is_error() {
    use std::os::unix::fs::symlink;

    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let pointer = state.path().join("runtimes/github-researcher/codex");
    fs::create_dir_all(pointer.parent().unwrap()).unwrap();
    symlink(
        "../../builds/codex/github-researcher/missing/home",
        &pointer,
    )
    .unwrap();

    let result = run_doctor(doctor_request(&repo, &state)).unwrap();

    let broken = diagnostic_with_code(&result.diagnostics, "runtime.pointer-target-invalid");
    assert_eq!(broken.severity, DiagnosticSeverity::Error);
    assert!(broken.message.contains("github-researcher"));
    assert!(broken.message.contains("codex"));
    assert!(result.has_error_diagnostics());
}

#[cfg(unix)]
#[test]
fn generated_runtime_pointer_under_builds_passes() {
    use std::os::unix::fs::symlink;

    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let home = state
        .path()
        .join("builds/codex/github-researcher/test-build/home");
    fs::create_dir_all(&home).unwrap();
    let pointer = state.path().join("runtimes/github-researcher/codex");
    fs::create_dir_all(pointer.parent().unwrap()).unwrap();
    symlink(
        "../../builds/codex/github-researcher/test-build/home",
        &pointer,
    )
    .unwrap();

    let result = run_doctor(doctor_request(&repo, &state)).unwrap();

    assert_eq!(result.generated_state.runtime_pointer_count, 1);
    assert_eq!(result.diagnostics, Vec::new());
}

#[cfg(unix)]
#[test]
fn runtime_pointer_to_unmanaged_directory_is_error() {
    use std::os::unix::fs::symlink;

    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let unmanaged = TempDir::new().unwrap();
    let unmanaged_home = unmanaged.path().join("home");
    fs::create_dir_all(&unmanaged_home).unwrap();
    let pointer = state.path().join("runtimes/github-researcher/codex");
    fs::create_dir_all(pointer.parent().unwrap()).unwrap();
    symlink(&unmanaged_home, &pointer).unwrap();

    let result = run_doctor(doctor_request(&repo, &state)).unwrap();

    let diagnostic = diagnostic_with_code(&result.diagnostics, "runtime.pointer-target-invalid");
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert!(diagnostic.message.contains("expected a generated home"));
    assert!(result.has_error_diagnostics());
}

#[cfg(unix)]
#[test]
fn non_writable_state_root_is_error() {
    use std::os::unix::fs::PermissionsExt;

    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let original = fs::metadata(state.path()).unwrap().permissions();
    let mut read_only = original.clone();
    read_only.set_mode(0o555);
    fs::set_permissions(state.path(), read_only).unwrap();

    let result = run_doctor(doctor_request(&repo, &state)).unwrap();
    fs::set_permissions(state.path(), original).unwrap();

    let diagnostic = diagnostic_with_code(&result.diagnostics, "runtime.state-root-not-writable");
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert!(result.has_error_diagnostics());
}

#[test]
fn missing_relative_state_root_uses_current_directory_writability() {
    let repo = valid_repo();
    let state = tempfile::Builder::new()
        .prefix("agent-matters-relative-state")
        .tempdir_in(".")
        .unwrap();
    let relative_state = PathBuf::from(state.path().file_name().unwrap().to_os_string());
    drop(state);

    let result = run_doctor(DoctorRequest {
        repo_root: repo.path().to_path_buf(),
        user_state_dir: relative_state,
        native_home_dir: None,
    })
    .unwrap();

    assert!(result.generated_state.writable);
    assert_eq!(result.diagnostics, Vec::new());
}

#[test]
fn missing_state_root_with_file_parent_is_error() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let blocker = state.path().join("not-a-directory");
    fs::write(&blocker, b"blocks nested state roots").unwrap();

    let result = run_doctor(DoctorRequest {
        repo_root: repo.path().to_path_buf(),
        user_state_dir: blocker.join("child-state"),
        native_home_dir: None,
    })
    .unwrap();

    let diagnostic = diagnostic_with_code(
        &result.diagnostics,
        "runtime.state-root-parent-not-directory",
    );
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert!(!result.generated_state.writable);
    assert!(result.has_error_diagnostics());
}

#[test]
fn fresh_install_with_no_builds_passes_generated_cache_checks() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let native_home = native_home_with_codex_auth(state.path());

    let result = run_doctor(doctor_request_with_native_home(&repo, &state, native_home)).unwrap();

    assert_eq!(result.generated_state.runtime_pointer_count, 0);
    assert_eq!(result.index.status, DoctorIndexStatus::Missing);
    assert_eq!(result.diagnostics, Vec::new());
}
