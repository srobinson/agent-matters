//! Runtime adapter contracts and built in adapter registry.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use agent_matters_core::config::RuntimeSettings;
use agent_matters_core::domain::{Diagnostic, DiagnosticLocation, DiagnosticSeverity};
use agent_matters_core::runtime::{
    CredentialSymlinkAllowlistEntry, RuntimeHomeFile, RuntimeLaunchInstructions,
};

use super::claude_adapter::ClaudeRuntimeAdapter;
use super::codex_adapter::CodexRuntimeAdapter;
use super::{AssembledProfileInstructions, ProfileBuildPlan, ResolvedRuntimeConfig};

pub const CODEX_RUNTIME_ID: &str = "codex";
pub const CLAUDE_RUNTIME_ID: &str = "claude";

static CLAUDE_ADAPTER: ClaudeRuntimeAdapter = ClaudeRuntimeAdapter;
static CODEX_ADAPTER: CodexRuntimeAdapter = CodexRuntimeAdapter;

pub trait RuntimeAdapter: Sync {
    fn id(&self) -> &'static str;

    fn version(&self) -> &'static str;

    fn default_settings(&self) -> RuntimeSettings {
        RuntimeSettings::default()
    }

    fn validate_config(&self, config: &ResolvedRuntimeConfig) -> Vec<Diagnostic> {
        if config.id == self.id() {
            return Vec::new();
        }

        vec![
            Diagnostic::new(
                DiagnosticSeverity::Error,
                "runtime.adapter.config-mismatch",
                format!(
                    "runtime adapter `{}` cannot validate config for runtime `{}`",
                    self.id(),
                    config.id
                ),
            )
            .with_location(DiagnosticLocation::manifest_field(
                Path::new("defaults/runtimes.toml"),
                format!("runtimes.{}", config.id),
            )),
        ]
    }

    fn render_home(&self, request: RuntimeHomeRenderRequest<'_>) -> RuntimeHomeRenderResult;

    fn credential_symlink_allowlist(&self) -> Vec<CredentialSymlinkAllowlistEntry> {
        Vec::new()
    }

    fn credential_source_dir(&self, native_home_dir: &Path) -> Option<PathBuf> {
        let _ = native_home_dir;
        None
    }

    fn launch_instructions(&self, request: RuntimeLaunchRequest<'_>) -> RuntimeLaunchInstructions;
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeHomeRenderRequest<'a> {
    pub repo_root: &'a Path,
    pub plan: &'a ProfileBuildPlan,
    pub instructions: &'a AssembledProfileInstructions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeHomeRenderResult {
    pub files: Vec<RuntimeHomeFile>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeLaunchRequest<'a> {
    pub runtime_home: &'a Path,
    pub workspace_path: &'a str,
}

pub fn runtime_adapters() -> [&'static dyn RuntimeAdapter; 2] {
    [&CLAUDE_ADAPTER, &CODEX_ADAPTER]
}

pub fn runtime_adapter_ids() -> Vec<&'static str> {
    runtime_adapters()
        .into_iter()
        .map(RuntimeAdapter::id)
        .collect()
}

pub fn adapter_for_runtime(runtime: &str) -> Option<&'static dyn RuntimeAdapter> {
    runtime_adapters()
        .into_iter()
        .find(|adapter| adapter.id() == runtime)
}

pub(super) fn launch_instructions(
    env_name: &str,
    runtime_home: &Path,
    args: Vec<String>,
) -> RuntimeLaunchInstructions {
    let runtime_home = path_string(runtime_home);
    let mut env = BTreeMap::new();
    env.insert(env_name.to_string(), runtime_home.clone());
    let command = format!(
        "{}={} {}",
        env_name,
        shell_quote(&runtime_home),
        shell_words(&args)
    );

    RuntimeLaunchInstructions { env, args, command }
}

fn shell_words(words: &[String]) -> String {
    words
        .iter()
        .map(|word| shell_quote(word))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_quote(value: &str) -> String {
    if value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b"@%_+=:,./-".contains(&byte))
    {
        return value.to_string();
    }

    format!("'{}'", value.replace('\'', "'\\''"))
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub fn unknown_runtime_adapter(runtime: &str) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "runtime.adapter.unknown",
        format!("no runtime adapter is registered for `{runtime}`"),
    )
    .with_recovery_hint("choose a runtime with a registered adapter")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy)]
    struct FixtureAdapter;

    impl RuntimeAdapter for FixtureAdapter {
        fn id(&self) -> &'static str {
            "fixture"
        }

        fn version(&self) -> &'static str {
            "agent-matters:fixture:adapter:v1"
        }

        fn render_home(&self, _request: RuntimeHomeRenderRequest<'_>) -> RuntimeHomeRenderResult {
            RuntimeHomeRenderResult {
                files: Vec::new(),
                diagnostics: Vec::new(),
            }
        }

        fn credential_symlink_allowlist(&self) -> Vec<CredentialSymlinkAllowlistEntry> {
            vec![CredentialSymlinkAllowlistEntry::new(
                "auth.json",
                "auth.json",
            )]
        }

        fn launch_instructions(
            &self,
            request: RuntimeLaunchRequest<'_>,
        ) -> RuntimeLaunchInstructions {
            launch_instructions(
                "FIXTURE_HOME",
                request.runtime_home,
                vec!["fixture".to_string(), request.workspace_path.to_string()],
            )
        }
    }

    #[test]
    fn fixture_adapter_proves_contract_surface() {
        let adapter = FixtureAdapter;
        let allowlist = adapter.credential_symlink_allowlist();

        assert_eq!(adapter.id(), "fixture");
        assert_eq!(adapter.version(), "agent-matters:fixture:adapter:v1");
        assert_eq!(adapter.default_settings(), RuntimeSettings::default());
        assert_eq!(allowlist[0].source_name, "auth.json");

        let launch = adapter.launch_instructions(RuntimeLaunchRequest {
            runtime_home: Path::new("/tmp/agent matters"),
            workspace_path: "/work/tree",
        });
        assert_eq!(launch.env["FIXTURE_HOME"], "/tmp/agent matters");
        assert_eq!(launch.args, vec!["fixture", "/work/tree"]);
        assert_eq!(
            launch.command,
            "FIXTURE_HOME='/tmp/agent matters' fixture /work/tree"
        );
    }
}
