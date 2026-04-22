use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use agent_matters_core::catalog::CapabilityIndexRecord;
use agent_matters_core::domain::Diagnostic;
use agent_matters_core::runtime::RuntimeHomeFile;
use serde_json::{Map, Value, json};

use crate::profiles::adapter::{RuntimeHomeRenderRequest, RuntimeHomeRenderResult};
use crate::profiles::adapter_capabilities::{
    RuntimeCapabilitySupport, capability_body, capability_target_path, path_string,
};

use super::{CLAUDE_JSON_FILE, INSTRUCTIONS_FILE};

const SETTINGS_FILE: &str = "settings.json";
const SKILLS_DIR: &str = "skills";
const HOOKS_DIR: &str = "hooks";
const AGENTS_DIR: &str = "agents";
const SUPPORT: RuntimeCapabilitySupport = RuntimeCapabilitySupport::new(
    "Claude",
    "runtime.claude",
    "Claude config",
    "Claude JSON config",
);

pub(super) fn render_claude_home(request: RuntimeHomeRenderRequest<'_>) -> RuntimeHomeRenderResult {
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

    fn add_capability(&mut self, capability: &CapabilityIndexRecord) {
        match capability.kind.as_str() {
            "skill" => self.add_single_file_capability(capability, "source", SKILLS_DIR),
            "hook" => self.add_hook(capability),
            "mcp" => self.add_mcp_server(capability),
            "agent" => self.add_agent(capability),
            "runtime-setting" | "instruction" => {}
            _ => self
                .diagnostics
                .push(SUPPORT.unsupported_capability_kind(capability)),
        }
    }

    fn add_single_file_capability(
        &mut self,
        capability: &CapabilityIndexRecord,
        role: &str,
        target_dir: &str,
    ) {
        let Some(source_file) =
            SUPPORT.expected_file_mapping(capability, role, &mut self.diagnostics)
        else {
            return;
        };
        let target_path = capability_target_path(capability, target_dir, source_file);
        self.copy_capability_file(capability, source_file, target_path);
    }

    fn add_hook(&mut self, capability: &CapabilityIndexRecord) {
        let Some(source_file) =
            SUPPORT.expected_file_mapping(capability, "script", &mut self.diagnostics)
        else {
            return;
        };
        let target_path = capability_target_path(capability, HOOKS_DIR, source_file);
        self.copy_capability_file(capability, source_file, target_path.clone());
        self.hooks.push(ClaudeHookRecord {
            command: format!("\"$CLAUDE_CONFIG_DIR\"/{}", path_string(&target_path)),
        });
    }

    fn add_agent(&mut self, capability: &CapabilityIndexRecord) {
        let Some(source_file) =
            SUPPORT.expected_file_mapping(capability, "instructions", &mut self.diagnostics)
        else {
            return;
        };
        let target_path =
            PathBuf::from(AGENTS_DIR).join(format!("{}.md", capability_body(&capability.id)));
        self.copy_capability_file(capability, source_file, target_path);
    }

    fn add_mcp_server(&mut self, capability: &CapabilityIndexRecord) {
        let Some(source_file) =
            SUPPORT.expected_file_mapping(capability, "manifest", &mut self.diagnostics)
        else {
            return;
        };
        let path = self.capability_source_path(capability, source_file);
        let encoded = match fs::read_to_string(&path) {
            Ok(encoded) => encoded,
            Err(source) => {
                self.diagnostics
                    .push(SUPPORT.file_read_failed(capability, source_file, &source));
                return;
            }
        };
        let parsed: toml::Value = match toml::from_str(&encoded) {
            Ok(parsed) => parsed,
            Err(source) => {
                self.diagnostics.push(SUPPORT.mcp_manifest_invalid(
                    capability,
                    source_file,
                    &source,
                ));
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
                self.diagnostics.push(SUPPORT.config_render_failed(&source));
                return;
            }
        };
        encoded.push('\n');
        self.files.push(RuntimeHomeFile::text(path, encoded));
    }

    fn copy_capability_file(
        &mut self,
        capability: &CapabilityIndexRecord,
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
                    .push(SUPPORT.file_read_failed(capability, source_file, &source))
            }
        }
    }

    fn capability_source_path(
        &self,
        capability: &CapabilityIndexRecord,
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
