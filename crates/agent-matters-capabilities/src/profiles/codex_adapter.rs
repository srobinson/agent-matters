//! Codex runtime home renderer.

mod renderer;

use std::path::{Path, PathBuf};

use agent_matters_core::runtime::{CredentialSymlinkAllowlistEntry, RuntimeLaunchInstructions};

use self::renderer::render_codex_home;
use super::adapter::{
    CODEX_RUNTIME_ID, RuntimeAdapter, RuntimeHomeRenderRequest, RuntimeHomeRenderResult,
    RuntimeLaunchRequest, launch_instructions,
};

const CODEX_NATIVE_DIR: &str = ".codex";

#[derive(Debug, Clone, Copy)]
pub(crate) struct CodexRuntimeAdapter;

impl RuntimeAdapter for CodexRuntimeAdapter {
    fn id(&self) -> &'static str {
        CODEX_RUNTIME_ID
    }

    fn version(&self) -> &'static str {
        "agent-matters:codex:adapter:v2"
    }

    fn render_home(&self, request: RuntimeHomeRenderRequest<'_>) -> RuntimeHomeRenderResult {
        render_codex_home(request)
    }

    fn credential_symlink_allowlist(&self) -> Vec<CredentialSymlinkAllowlistEntry> {
        vec![CredentialSymlinkAllowlistEntry::new(
            "auth.json",
            "auth.json",
        )]
    }

    fn credential_source_dir(&self, native_home_dir: &Path) -> Option<PathBuf> {
        Some(native_home_dir.join(CODEX_NATIVE_DIR))
    }

    fn launch_instructions(&self, request: RuntimeLaunchRequest<'_>) -> RuntimeLaunchInstructions {
        launch_instructions(
            "CODEX_HOME",
            request.runtime_home,
            vec![
                "codex".to_string(),
                "-C".to_string(),
                request.workspace_path.to_string(),
            ],
        )
    }
}
