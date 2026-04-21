use agent_matters_capabilities::profiles::compile_profile_build;
use agent_matters_core::domain::DiagnosticSeverity;
use tempfile::TempDir;

use crate::common::{append_requires, compile_request, set_profile_runtimes, valid_repo};

#[test]
fn compile_warns_on_missing_required_env_and_still_writes_build() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    append_requires(
        repo.path(),
        "catalog/mcp/linear/manifest.toml",
        "env = [\"LINEAR_API_KEY\"]\n",
    );

    let result = compile_profile_build(compile_request(repo.path(), state.path())).unwrap();

    assert!(result.build.is_some());
    assert!(!result.has_error_diagnostics());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].severity, DiagnosticSeverity::Warning);
    assert_eq!(result.diagnostics[0].code, "profile.required-env-missing");
}

#[test]
fn compile_fails_on_missing_required_capability() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    append_requires(
        repo.path(),
        "catalog/mcp/linear/manifest.toml",
        "capabilities = [\"mcp:context-matters\"]\n",
    );

    let result = compile_profile_build(compile_request(repo.path(), state.path())).unwrap();

    assert!(result.build.is_none());
    assert!(result.has_error_diagnostics());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].code,
        "profile.required-capability-missing"
    );
}

#[test]
fn compile_fails_on_runtime_not_enabled_for_profile() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let mut request = compile_request(repo.path(), state.path());
    request.runtime = Some("claude".to_string());

    let result = compile_profile_build(request).unwrap();

    assert!(result.build.is_none());
    assert!(result.has_error_diagnostics());
    assert_eq!(
        result.diagnostics[0].code,
        "profile.build-plan.runtime-unavailable"
    );
}

#[test]
fn compile_fails_when_capability_does_not_support_requested_runtime() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    set_profile_runtimes(
        repo.path(),
        r#"[runtimes.codex]
enabled = true

[runtimes.claude]
enabled = true
"#,
    );
    let mut request = compile_request(repo.path(), state.path());
    request.runtime = Some("claude".to_string());

    let result = compile_profile_build(request).unwrap();

    assert!(result.build.is_none());
    assert!(result.has_error_diagnostics());
    assert_eq!(
        result.diagnostics[0].code,
        "profile.runtime.capability-unsupported"
    );
}
