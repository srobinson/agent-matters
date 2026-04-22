//! Claude runtime home renderer.

mod renderer;

use std::path::{Path, PathBuf};

use agent_matters_core::runtime::{
    CredentialSymlinkAllowlistEntry, RuntimeHomeFile, RuntimeLaunchInstructions,
};
use serde_json::Value;

use self::renderer::render_claude_home;
use super::adapter::{
    CLAUDE_RUNTIME_ID, RuntimeAdapter, RuntimeHomeRenderRequest, RuntimeHomeRenderResult,
    RuntimeLaunchRequest, launch_instructions,
};

const CLAUDE_JSON_FILE: &str = ".claude.json";
const INSTRUCTIONS_FILE: &str = "CLAUDE.md";
const CLAUDE_NATIVE_DIR: &str = ".claude";

#[derive(Debug, Clone, Copy)]
pub(crate) struct ClaudeRuntimeAdapter;

impl RuntimeAdapter for ClaudeRuntimeAdapter {
    fn id(&self) -> &'static str {
        CLAUDE_RUNTIME_ID
    }

    fn version(&self) -> &'static str {
        "agent-matters:claude:adapter:v2"
    }

    fn render_home(&self, request: RuntimeHomeRenderRequest<'_>) -> RuntimeHomeRenderResult {
        render_claude_home(request)
    }

    fn existing_home_file_matches(&self, file: &RuntimeHomeFile, existing: &[u8]) -> bool {
        if file.relative_path == Path::new(CLAUDE_JSON_FILE) {
            return claude_json_mcp_servers_match(&file.contents, existing);
        }

        file.contents == existing
    }

    fn credential_symlink_allowlist(&self) -> Vec<CredentialSymlinkAllowlistEntry> {
        vec![CredentialSymlinkAllowlistEntry::new(
            ".credentials.json",
            ".credentials.json",
        )]
    }

    fn credential_source_dir(&self, native_home_dir: &Path) -> Option<PathBuf> {
        Some(native_home_dir.join(CLAUDE_NATIVE_DIR))
    }

    fn launch_instructions(&self, request: RuntimeLaunchRequest<'_>) -> RuntimeLaunchInstructions {
        launch_instructions(
            "CLAUDE_CONFIG_DIR",
            request.runtime_home,
            vec!["claude".to_string(), request.workspace_path.to_string()],
        )
    }
}

fn claude_json_mcp_servers_match(expected: &[u8], existing: &[u8]) -> bool {
    let Ok(expected): Result<Value, _> = serde_json::from_slice(expected) else {
        return false;
    };
    let Ok(existing): Result<Value, _> = serde_json::from_slice(existing) else {
        return false;
    };
    let Some(expected_servers) = expected.get("mcpServers") else {
        return false;
    };

    existing.get("mcpServers") == Some(expected_servers)
}
