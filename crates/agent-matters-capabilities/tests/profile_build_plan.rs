mod support;

use std::fs;
use std::path::Path;

use agent_matters_capabilities::profiles::{
    BuildProfilePlanRequest, ProfileBuildPlan, adapter_for_runtime, plan_profile_build,
};
use agent_matters_core::domain::DiagnosticSeverity;
use serde_json::{Value, json};
use tempfile::TempDir;

use support::fixture_path;

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let source = entry.path();
        let target = to.join(entry.file_name());
        if source.is_dir() {
            copy_dir(&source, &target);
        } else {
            fs::copy(&source, &target).unwrap();
        }
    }
}

fn valid_repo() -> TempDir {
    let tmp = TempDir::new().unwrap();
    copy_dir(&fixture_path("catalogs/valid"), tmp.path());
    tmp
}

fn plan(repo_root: &Path, state: &Path) -> ProfileBuildPlan {
    let result = plan_profile_build(BuildProfilePlanRequest {
        repo_root: repo_root.to_path_buf(),
        user_state_dir: state.to_path_buf(),
        profile: "github-researcher".to_string(),
        runtime: Some("codex".to_string()),
    })
    .unwrap();

    assert_eq!(result.diagnostics, Vec::new());
    result.plan.unwrap()
}

fn write(root: &Path, rel: &str, body: &str) {
    let full = root.join(rel);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(full, body).unwrap();
}

fn set_profile_runtimes(repo: &Path, runtimes: &str) {
    let path = repo.join("profiles/renamed-profile-dir/manifest.toml");
    let body = fs::read_to_string(&path).unwrap();
    let prefix = body.split("[runtimes.codex]").next().unwrap();
    fs::write(path, format!("{prefix}{runtimes}")).unwrap();
}

#[test]
fn same_resolved_content_has_same_fingerprint_in_different_roots() {
    let first_repo = valid_repo();
    let second_repo = valid_repo();
    let first_state = TempDir::new().unwrap();
    let second_state = TempDir::new().unwrap();

    let first = plan(first_repo.path(), first_state.path());
    let second = plan(second_repo.path(), second_state.path());

    assert_eq!(first.fingerprint, second.fingerprint);
    assert_eq!(first.build_id, second.build_id);
    assert_eq!(first.paths, second.paths);
}

#[test]
fn fingerprint_changes_on_profile_manifest_change() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let first = plan(repo.path(), state.path());

    let manifest_path = repo
        .path()
        .join("profiles/renamed-profile-dir/manifest.toml");
    let updated = fs::read_to_string(&manifest_path)
        .unwrap()
        .replace("Focused research agent", "Focused issue research agent");
    fs::write(manifest_path, updated).unwrap();

    let second = plan(repo.path(), state.path());

    assert_ne!(first.fingerprint, second.fingerprint);
}

#[test]
fn fingerprint_changes_on_included_instruction_file_change() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let first = plan(repo.path(), state.path());

    write(
        repo.path(),
        "catalog/instructions/helioy-core/AGENTS.md",
        "# Helioy Core\n\nUpdated instruction content.\n",
    );
    let second = plan(repo.path(), state.path());

    assert_ne!(first.fingerprint, second.fingerprint);
}

#[test]
fn fingerprint_excludes_unrelated_environment_values() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let first = plan(repo.path(), state.path());

    unsafe {
        std::env::set_var("AGENT_MATTERS_UNRELATED_TEST_ENV", "changed");
    }
    let second = plan(repo.path(), state.path());
    unsafe {
        std::env::remove_var("AGENT_MATTERS_UNRELATED_TEST_ENV");
    }

    assert_eq!(first.fingerprint, second.fingerprint);
}

#[test]
fn adapter_version_is_read_from_registered_runtime_adapter() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let build_plan = plan(repo.path(), state.path());
    let adapter = adapter_for_runtime(&build_plan.runtime).unwrap();

    assert_eq!(build_plan.adapter_version, adapter.version());
    assert_eq!(build_plan.adapter_version, "agent-matters:codex:adapter:v1");
    assert_eq!(build_plan.fingerprint, "fnv64:cd7453c6604d912f");
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

    let result = plan_profile_build(BuildProfilePlanRequest {
        repo_root: repo.path().to_path_buf(),
        user_state_dir: state.path().to_path_buf(),
        profile: "github-researcher".to_string(),
        runtime: Some("codex".to_string()),
    })
    .unwrap();

    assert_eq!(result.diagnostics, Vec::new());
    assert_eq!(result.plan.unwrap().runtime, "codex");
}

#[test]
fn missing_referenced_content_file_reports_input_diagnostic() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    fs::remove_file(
        repo.path()
            .join("catalog/instructions/helioy-core/AGENTS.md"),
    )
    .unwrap();

    let result = plan_profile_build(BuildProfilePlanRequest {
        repo_root: repo.path().to_path_buf(),
        user_state_dir: state.path().to_path_buf(),
        profile: "github-researcher".to_string(),
        runtime: Some("codex".to_string()),
    })
    .unwrap();

    assert!(result.plan.is_none());
    assert_eq!(result.diagnostics.len(), 1);
    let diagnostic = &result.diagnostics[0];
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(diagnostic.code, "profile.build-plan.input-read-failed");
    assert!(
        diagnostic
            .message
            .contains("catalog/instructions/helioy-core/AGENTS.md")
    );
    assert_eq!(
        diagnostic
            .location
            .as_ref()
            .and_then(|location| location.manifest_path.as_deref()),
        Some(Path::new("catalog/instructions/helioy-core/AGENTS.md"))
    );
}

#[test]
fn build_plan_json_shape_is_stable() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    let encoded = serde_json::to_value(plan(repo.path(), state.path())).unwrap();

    assert_eq!(encoded, expected_build_plan_json());
}

fn expected_build_plan_json() -> Value {
    json!({
        "schema_version": 1,
        "profile": "github-researcher",
        "runtime": "codex",
        "adapter_version": "agent-matters:codex:adapter:v1",
        "fingerprint": "fnv64:cd7453c6604d912f",
        "build_id": "cd7453c6604d912f",
        "paths": {
            "build_dir": "builds/codex/github-researcher/cd7453c6604d912f",
            "home_dir": "builds/codex/github-researcher/cd7453c6604d912f/home",
            "runtime_pointer": "runtimes/github-researcher/codex"
        },
        "profile_record": profile_record_json(),
        "effective_capabilities": effective_capabilities_json(),
        "instruction_fragments": [
            {
                "id": "instruction:helioy-core",
                "kind": "instruction",
                "source_path": "catalog/instructions/helioy-core",
                "files": {
                    "content": "AGENTS.md"
                }
            },
            {
                "id": "agent:github-researcher",
                "kind": "agent",
                "source_path": "catalog/agents/github-researcher",
                "files": {
                    "instructions": "AGENT.md"
                }
            }
        ],
        "instruction_output": {
            "markers": "html-comments"
        },
        "runtime_config": {
            "id": "codex"
        },
        "content_inputs": content_inputs_json()
    })
}

fn effective_capabilities_json() -> Value {
    json!([
        local_capability(
            "skill:playwright",
            "skill",
            "Playwright browser automation skill.",
            "source",
            "SKILL.md",
            "catalog/skills/renamed-skill-dir",
        ),
        local_capability(
            "mcp:linear",
            "mcp",
            "Linear MCP server.",
            "manifest",
            "server.toml",
            "catalog/mcp/linear",
        ),
        local_capability(
            "hook:session-logger",
            "hook",
            "Session handover hook.",
            "script",
            "hook.sh",
            "catalog/hooks/session-logger",
        ),
        local_capability(
            "runtime-setting:codex-defaults",
            "runtime-setting",
            "Codex runtime defaults.",
            "settings",
            "config.toml",
            "catalog/runtime-settings/codex-defaults",
        ),
        local_capability(
            "instruction:helioy-core",
            "instruction",
            "Core Helioy operating instructions.",
            "content",
            "AGENTS.md",
            "catalog/instructions/helioy-core",
        ),
        local_capability(
            "agent:github-researcher",
            "agent",
            "GitHub research specialist agent.",
            "instructions",
            "AGENT.md",
            "catalog/agents/github-researcher",
        ),
    ])
}

fn content_inputs_json() -> Value {
    json!([
        content_input(
            "profile-manifest",
            "profiles/renamed-profile-dir/manifest.toml",
            "fnv64:6796138a39687371",
        ),
        content_input(
            "capability-manifest",
            "catalog/agents/github-researcher/manifest.toml",
            "fnv64:80d487270fab45ac",
        ),
        content_input(
            "capability-manifest",
            "catalog/hooks/session-logger/manifest.toml",
            "fnv64:59e2fd65b6b7c50d",
        ),
        content_input(
            "capability-manifest",
            "catalog/instructions/helioy-core/manifest.toml",
            "fnv64:ee5f5e0ffb700287",
        ),
        content_input(
            "capability-manifest",
            "catalog/mcp/linear/manifest.toml",
            "fnv64:8bf04d99ba6800f4",
        ),
        content_input(
            "capability-manifest",
            "catalog/runtime-settings/codex-defaults/manifest.toml",
            "fnv64:e0ba7e4251b914fc",
        ),
        content_input(
            "capability-manifest",
            "catalog/skills/renamed-skill-dir/manifest.toml",
            "fnv64:3638056d9550bf9e",
        ),
        content_input(
            "capability-file",
            "catalog/agents/github-researcher/AGENT.md",
            "fnv64:ab8da577f565b227",
        ),
        content_input(
            "capability-file",
            "catalog/hooks/session-logger/hook.sh",
            "fnv64:e82673ea83700a66",
        ),
        content_input(
            "capability-file",
            "catalog/instructions/helioy-core/AGENTS.md",
            "fnv64:a2cf8bcab5d06541",
        ),
        content_input(
            "capability-file",
            "catalog/mcp/linear/server.toml",
            "fnv64:aeddd797bea0b263",
        ),
        content_input(
            "capability-file",
            "catalog/runtime-settings/codex-defaults/config.toml",
            "fnv64:cbb301a15a1cf3c2",
        ),
        content_input(
            "capability-file",
            "catalog/skills/renamed-skill-dir/SKILL.md",
            "fnv64:daed9f9bc506357f",
        ),
    ])
}

fn content_input(role: &str, path: &str, content_fingerprint: &str) -> Value {
    json!({
        "role": role,
        "path": path,
        "content_fingerprint": content_fingerprint
    })
}

fn profile_record_json() -> Value {
    json!({
        "id": "github-researcher",
        "kind": "persona",
        "summary": "Focused research agent for inspecting GitHub repositories.",
        "capabilities": [
            "skill:playwright",
            "mcp:linear",
            "hook:session-logger",
            "runtime-setting:codex-defaults"
        ],
        "instructions": [
            "instruction:helioy-core",
            "agent:github-researcher"
        ],
        "source_path": "profiles/renamed-profile-dir",
        "runtimes": {
            "codex": {
                "supported": true
            }
        },
        "capability_count": 4,
        "instruction_count": 2
    })
}

fn local_capability(
    id: &str,
    kind: &str,
    summary: &str,
    file_key: &str,
    file_path: &str,
    source_path: &str,
) -> Value {
    json!({
        "id": id,
        "kind": kind,
        "summary": summary,
        "files": {
            file_key: file_path
        },
        "source_path": source_path,
        "source": {
            "kind": "local",
            "normalized_path": source_path
        },
        "runtimes": {
            "codex": {
                "supported": true
            }
        },
        "provenance": {
            "kind": "local"
        },
        "requirements": {}
    })
}
