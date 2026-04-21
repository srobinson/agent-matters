//! Claude runtime home renderer.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use agent_matters_core::catalog::MANIFEST_FILE_NAME;
use agent_matters_core::domain::{Diagnostic, DiagnosticLocation, DiagnosticSeverity};
use agent_matters_core::runtime::{
    CredentialSymlinkAllowlistEntry, RuntimeHomeFile, RuntimeLaunchInstructions,
};
use serde_json::{Map, Value, json};

use super::adapter::{
    CLAUDE_RUNTIME_ID, RuntimeAdapter, RuntimeHomeRenderRequest, RuntimeHomeRenderResult,
    RuntimeLaunchRequest, launch_instructions,
};

const CLAUDE_JSON_FILE: &str = ".claude.json";
const SETTINGS_FILE: &str = "settings.json";
const INSTRUCTIONS_FILE: &str = "CLAUDE.md";
const SKILLS_DIR: &str = "skills";
const HOOKS_DIR: &str = "hooks";
const AGENTS_DIR: &str = "agents";
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
        let mut renderer = ClaudeHomeRenderer::new(request.repo_root);
        renderer.files.push(RuntimeHomeFile::text(
            INSTRUCTIONS_FILE,
            request.instructions.content.clone(),
        ));

        for capability in &request.plan.effective_capabilities {
            renderer.add_capability(capability);
        }
        renderer.add_settings(&request.plan.runtime_config.model);
        renderer.add_mcp_config();
        renderer.finish()
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

#[derive(Debug)]
struct ClaudeHomeRenderer<'a> {
    repo_root: &'a Path,
    files: Vec<RuntimeHomeFile>,
    diagnostics: Vec<Diagnostic>,
    mcp_servers: BTreeMap<String, Value>,
    hooks: Vec<ClaudeHookRecord>,
}

impl<'a> ClaudeHomeRenderer<'a> {
    fn new(repo_root: &'a Path) -> Self {
        Self {
            repo_root,
            files: Vec::new(),
            diagnostics: Vec::new(),
            mcp_servers: BTreeMap::new(),
            hooks: Vec::new(),
        }
    }

    fn add_capability(&mut self, capability: &agent_matters_core::catalog::CapabilityIndexRecord) {
        match capability.kind.as_str() {
            "skill" => self.add_single_file_capability(capability, "source", SKILLS_DIR),
            "hook" => self.add_hook(capability),
            "mcp" => self.add_mcp_server(capability),
            "agent" => self.add_agent(capability),
            "runtime-setting" | "instruction" => {}
            _ => self
                .diagnostics
                .push(unsupported_capability_kind(capability)),
        }
    }

    fn add_single_file_capability(
        &mut self,
        capability: &agent_matters_core::catalog::CapabilityIndexRecord,
        role: &str,
        target_dir: &str,
    ) {
        let Some(source_file) = expected_file_mapping(capability, role, &mut self.diagnostics)
        else {
            return;
        };
        let target_path = capability_target_path(capability, target_dir, source_file);
        self.copy_capability_file(capability, source_file, target_path);
    }

    fn add_hook(&mut self, capability: &agent_matters_core::catalog::CapabilityIndexRecord) {
        let Some(source_file) = expected_file_mapping(capability, "script", &mut self.diagnostics)
        else {
            return;
        };
        let target_path = capability_target_path(capability, HOOKS_DIR, source_file);
        self.copy_capability_file(capability, source_file, target_path.clone());
        self.hooks.push(ClaudeHookRecord {
            command: format!("\"$CLAUDE_CONFIG_DIR\"/{}", path_string(&target_path)),
        });
    }

    fn add_agent(&mut self, capability: &agent_matters_core::catalog::CapabilityIndexRecord) {
        let Some(source_file) =
            expected_file_mapping(capability, "instructions", &mut self.diagnostics)
        else {
            return;
        };
        let target_path =
            PathBuf::from(AGENTS_DIR).join(format!("{}.md", capability_body(&capability.id)));
        self.copy_capability_file(capability, source_file, target_path);
    }

    fn add_mcp_server(&mut self, capability: &agent_matters_core::catalog::CapabilityIndexRecord) {
        let Some(source_file) =
            expected_file_mapping(capability, "manifest", &mut self.diagnostics)
        else {
            return;
        };
        let path = self.capability_source_path(capability, source_file);
        let encoded = match fs::read_to_string(&path) {
            Ok(encoded) => encoded,
            Err(source) => {
                self.diagnostics
                    .push(file_read_failed(capability, source_file, &source));
                return;
            }
        };
        let parsed: toml::Value = match toml::from_str(&encoded) {
            Ok(parsed) => parsed,
            Err(source) => {
                self.diagnostics
                    .push(mcp_manifest_invalid(capability, source_file, &source));
                return;
            }
        };
        let mut value = serde_json::to_value(parsed).expect("TOML values serialize as JSON");
        add_default_stdio_args(&mut value);
        self.mcp_servers
            .insert(capability_body(&capability.id).to_string(), value);
    }

    fn add_settings(&mut self, model: &Option<String>) {
        let mut settings = Map::new();
        if let Some(model) = model {
            settings.insert("model".to_string(), Value::String(model.clone()));
        }
        if !self.hooks.is_empty() {
            settings.insert(
                "hooks".to_string(),
                hooks_settings(std::mem::take(&mut self.hooks)),
            );
        }
        self.add_json_file(SETTINGS_FILE, Value::Object(settings));
    }

    fn add_mcp_config(&mut self) {
        if self.mcp_servers.is_empty() {
            return;
        }
        let mut root = Map::new();
        root.insert(
            "mcpServers".to_string(),
            Value::Object(std::mem::take(&mut self.mcp_servers).into_iter().collect()),
        );
        self.add_json_file(CLAUDE_JSON_FILE, Value::Object(root));
    }

    fn add_json_file(&mut self, path: &str, value: Value) {
        let mut encoded = match serde_json::to_string_pretty(&value) {
            Ok(encoded) => encoded,
            Err(source) => {
                self.diagnostics.push(config_render_failed(&source));
                return;
            }
        };
        encoded.push('\n');
        self.files.push(RuntimeHomeFile::text(path, encoded));
    }

    fn copy_capability_file(
        &mut self,
        capability: &agent_matters_core::catalog::CapabilityIndexRecord,
        source_file: &str,
        target_path: PathBuf,
    ) {
        let source_path = self.capability_source_path(capability, source_file);
        match fs::read(&source_path) {
            Ok(contents) => self.files.push(RuntimeHomeFile {
                relative_path: target_path,
                contents,
            }),
            Err(source) => {
                self.diagnostics
                    .push(file_read_failed(capability, source_file, &source))
            }
        }
    }

    fn capability_source_path(
        &self,
        capability: &agent_matters_core::catalog::CapabilityIndexRecord,
        file_path: &str,
    ) -> PathBuf {
        self.repo_root.join(&capability.source_path).join(file_path)
    }

    fn finish(self) -> RuntimeHomeRenderResult {
        RuntimeHomeRenderResult {
            files: self.files,
            diagnostics: self.diagnostics,
        }
    }
}

#[derive(Debug)]
struct ClaudeHookRecord {
    command: String,
}

fn hooks_settings(hooks: Vec<ClaudeHookRecord>) -> Value {
    Value::Object(
        [(
            "SessionEnd".to_string(),
            Value::Array(
                hooks
                    .into_iter()
                    .map(
                        |hook| json!({ "hooks": [{ "type": "command", "command": hook.command }] }),
                    )
                    .collect(),
            ),
        )]
        .into_iter()
        .collect(),
    )
}

fn add_default_stdio_args(value: &mut Value) {
    if let Value::Object(server) = value
        && server.contains_key("command")
        && !server.contains_key("args")
    {
        server.insert("args".to_string(), Value::Array(Vec::new()));
    }
}

fn expected_file_mapping<'a>(
    capability: &'a agent_matters_core::catalog::CapabilityIndexRecord,
    expected_role: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<&'a str> {
    for role in capability.files.keys() {
        if role != expected_role {
            diagnostics.push(unsupported_file_mapping(capability, role, expected_role));
        }
    }

    capability
        .files
        .get(expected_role)
        .map(String::as_str)
        .or_else(|| {
            diagnostics.push(missing_file_mapping(capability, expected_role));
            None
        })
}

fn capability_target_path(
    capability: &agent_matters_core::catalog::CapabilityIndexRecord,
    target_dir: &str,
    source_file: &str,
) -> PathBuf {
    PathBuf::from(target_dir)
        .join(capability_body(&capability.id))
        .join(source_file)
}

fn capability_body(id: &str) -> &str {
    id.split_once(':').map_or(id, |(_, body)| body)
}

fn capability_manifest_path(
    capability: &agent_matters_core::catalog::CapabilityIndexRecord,
) -> PathBuf {
    Path::new(&capability.source_path).join(MANIFEST_FILE_NAME)
}

fn path_string(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn unsupported_capability_kind(
    capability: &agent_matters_core::catalog::CapabilityIndexRecord,
) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "runtime.claude.capability-kind-unsupported",
        format!(
            "Claude adapter does not support capability kind `{}` for `{}`",
            capability.kind, capability.id
        ),
    )
    .with_location(DiagnosticLocation::manifest_path(capability_manifest_path(
        capability,
    )))
}

fn unsupported_file_mapping(
    capability: &agent_matters_core::catalog::CapabilityIndexRecord,
    actual_role: &str,
    expected_role: &str,
) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "runtime.claude.file-mapping-unsupported",
        format!(
            "Claude adapter does not support `{}` file mapping `{}`; expected `{}`",
            capability.id, actual_role, expected_role
        ),
    )
    .with_location(DiagnosticLocation::manifest_field(
        capability_manifest_path(capability),
        format!("files.{actual_role}"),
    ))
    .with_recovery_hint("use the supported file role for this capability kind")
}

fn missing_file_mapping(
    capability: &agent_matters_core::catalog::CapabilityIndexRecord,
    expected_role: &str,
) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "runtime.claude.file-mapping-missing",
        format!(
            "Claude adapter needs `{}` file mapping `{}`",
            capability.id, expected_role
        ),
    )
    .with_location(DiagnosticLocation::manifest_path(capability_manifest_path(
        capability,
    )))
}

fn file_read_failed(
    capability: &agent_matters_core::catalog::CapabilityIndexRecord,
    source_file: &str,
    source: &std::io::Error,
) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "runtime.claude.file-read-failed",
        format!(
            "failed to read `{}` file `{}` for Claude adapter: {source}",
            capability.id, source_file
        ),
    )
    .with_location(DiagnosticLocation::manifest_path(
        Path::new(&capability.source_path).join(source_file),
    ))
}

fn mcp_manifest_invalid(
    capability: &agent_matters_core::catalog::CapabilityIndexRecord,
    source_file: &str,
    source: &toml::de::Error,
) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "runtime.claude.mcp-config-invalid",
        format!(
            "failed to parse `{}` MCP file `{}` for Claude config: {source}",
            capability.id, source_file
        ),
    )
    .with_location(DiagnosticLocation::manifest_path(
        Path::new(&capability.source_path).join(source_file),
    ))
}

fn config_render_failed(source: &serde_json::Error) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "runtime.claude.config-render-failed",
        format!("failed to render Claude JSON config: {source}"),
    )
}
