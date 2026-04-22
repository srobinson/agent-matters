use serde_json::{Value, json};
use tempfile::TempDir;

use crate::common::{plan, valid_repo};

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
        "adapter_version": "agent-matters:codex:adapter:v2",
        "fingerprint": "fnv64:1d9e35a63b67fe88",
        "build_id": "1d9e35a63b67fe88",
        "paths": {
            "build_dir": "builds/codex/github-researcher/1d9e35a63b67fe88",
            "home_dir": "builds/codex/github-researcher/1d9e35a63b67fe88/home",
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
