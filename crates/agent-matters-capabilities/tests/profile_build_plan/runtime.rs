use agent_matters_capabilities::profiles::{adapter_for_runtime, plan_profile_build};
use tempfile::TempDir;

use crate::common::{build_plan_request, plan, set_profile_runtimes, valid_repo};

#[test]
fn adapter_version_is_read_from_registered_runtime_adapter() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let build_plan = plan(repo.path(), state.path());
    let adapter = adapter_for_runtime(&build_plan.runtime).unwrap();

    assert_eq!(build_plan.adapter_version, adapter.version());
    assert_eq!(build_plan.adapter_version, "agent-matters:codex:adapter:v2");
    assert_eq!(build_plan.fingerprint, "fnv64:1d9e35a63b67fe88");
}

#[test]
fn requested_runtime_bypasses_default_runtime_ambiguity() {
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

    let result = plan_profile_build(build_plan_request(repo.path(), state.path())).unwrap();

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(result.plan.unwrap().runtime, "codex");
}
