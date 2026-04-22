mod support;

use std::fs;
use std::path::Path;

use agent_matters_capabilities::profiles::{
    ListProfilesRequest, ShowProfileRequest, ShowProfileResult, list_profiles, show_profile,
};
use agent_matters_core::catalog::ProfileIndexRecord;
use agent_matters_core::domain::Diagnostic;
use serde_json::{Value, json};
use tempfile::TempDir;

use support::fixtures::fixture_path;

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let source = entry.path();
        let target = to.join(entry.file_name());
        if source.is_dir() {
            copy_dir(&source, &target);
        } else {
            fs::copy(source, target).unwrap();
        }
    }
}

fn has_code(diagnostics: &[Diagnostic], code: &str) -> bool {
    diagnostics.iter().any(|diagnostic| diagnostic.code == code)
}

#[test]
fn list_fixture_profiles_include_runtime_scope_and_summary() {
    let profile = list_fixture("catalogs/valid")
        .into_iter()
        .find(|profile| profile.id == "github-researcher")
        .unwrap();

    assert_eq!(profile.kind, "persona");
    assert_eq!(
        profile.summary,
        "Focused research agent for inspecting GitHub repositories."
    );
    assert_eq!(profile.scope.enforcement.as_str(), "none");
    assert!(profile.runtimes["codex"].supported);
}

#[test]
fn show_valid_profile_returns_resolved_inventory() {
    let result = show_fixture("catalogs/valid", "github-researcher");
    let record = result.record.as_ref().unwrap();

    assert_eq!(record.source_path, "profiles/renamed-profile-dir");
    assert_eq!(result.selected_runtime.as_deref(), Some("codex"));
    assert_eq!(
        result
            .effective_capabilities
            .iter()
            .map(|record| record.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "skill:playwright",
            "mcp:linear",
            "hook:session-logger",
            "runtime-setting:codex-defaults",
            "instruction:helioy-core",
            "agent:github-researcher",
        ]
    );
    assert_eq!(
        result
            .instruction_fragments
            .iter()
            .map(|fragment| fragment.id.as_str())
            .collect::<Vec<_>>(),
        vec!["instruction:helioy-core", "agent:github-researcher"]
    );
    assert_eq!(result.diagnostics, Vec::new());
}

#[test]
fn missing_profile_returns_actionable_error() {
    let result = show_fixture("catalogs/valid", "missing-profile");

    assert!(result.record.is_none());
    assert!(result.has_error_diagnostics());
    assert!(has_code(&result.diagnostics, "profile.resolve-not-found"));
    assert_eq!(
        result.diagnostics[0].recovery_hint.as_deref(),
        Some("run `agent-matters profiles list` to inspect exact profile ids")
    );
}

#[test]
fn show_profile_with_missing_capability_returns_diagnostic() {
    let repo = TempDir::new().unwrap();
    copy_dir(&fixture_path("catalogs/valid"), repo.path());
    let manifest_path = repo
        .path()
        .join("profiles/renamed-profile-dir/manifest.toml");
    let updated = fs::read_to_string(&manifest_path)
        .unwrap()
        .replace("skill:playwright", "skill:missing");
    fs::write(&manifest_path, updated).unwrap();

    let result = show_temp(repo.path(), "github-researcher");

    assert!(result.has_error_diagnostics());
    assert!(has_code(
        &result.diagnostics,
        "profile.capability-not-found"
    ));
    assert!(
        !result
            .effective_capabilities
            .iter()
            .any(|record| record.id == "skill:missing")
    );
}

#[test]
fn list_json_shape_is_stable() {
    let profiles = list_fixture("catalogs/valid");
    let encoded = serde_json::to_value(&profiles).unwrap();

    assert_eq!(encoded, json!([profile_record_json()]));
}

#[test]
fn show_json_shape_is_stable() {
    let result = show_fixture("catalogs/valid", "github-researcher");
    let encoded = serde_json::to_value(&result).unwrap();

    assert_eq!(
        encoded,
        json!({
            "profile": "github-researcher",
            "record": profile_record_json(),
            "effective_capabilities": [
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
            ],
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
            "runtime_configs": [
                {
                    "id": "codex"
                }
            ],
            "selected_runtime": "codex",
            "diagnostics": []
        })
    );
}

fn list_fixture(relative: &str) -> Vec<ProfileIndexRecord> {
    let state = TempDir::new().unwrap();
    list_profiles(ListProfilesRequest {
        repo_root: fixture_path(relative),
        user_state_dir: state.path().to_path_buf(),
    })
    .unwrap()
    .profiles
}

fn show_fixture(relative: &str, profile: &str) -> ShowProfileResult {
    let state = TempDir::new().unwrap();
    show_profile(ShowProfileRequest {
        repo_root: fixture_path(relative),
        user_state_dir: state.path().to_path_buf(),
        profile: profile.to_string(),
    })
    .unwrap()
}

fn show_temp(repo_root: &Path, profile: &str) -> ShowProfileResult {
    let state = TempDir::new().unwrap();
    show_profile(ShowProfileRequest {
        repo_root: repo_root.to_path_buf(),
        user_state_dir: state.path().to_path_buf(),
        profile: profile.to_string(),
    })
    .unwrap()
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
