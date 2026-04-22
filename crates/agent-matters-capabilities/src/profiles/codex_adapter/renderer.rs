use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use agent_matters_core::catalog::CapabilityIndexRecord;
use agent_matters_core::domain::Diagnostic;
use agent_matters_core::runtime::RuntimeHomeFile;
use serde::Serialize;
use toml::Value;

use crate::profiles::adapter::{RuntimeHomeRenderRequest, RuntimeHomeRenderResult};
use crate::profiles::adapter_capabilities::{
    RuntimeCapabilitySupport, capability_body, capability_target_path, path_string,
};

const CONFIG_FILE: &str = "config.toml";
const HOOKS_FILE: &str = "hooks.json";
const SKILLS_DIR: &str = "skills";
const HOOKS_DIR: &str = "hooks";
const SUPPORT: RuntimeCapabilitySupport = RuntimeCapabilitySupport::new(
    "Codex",
    "runtime.codex",
    "Codex config",
    "Codex config.toml",
);

pub(super) fn render_codex_home(request: RuntimeHomeRenderRequest<'_>) -> RuntimeHomeRenderResult {
    let mut renderer = CodexHomeRenderer::new(request.repo_root);
    renderer.files.push(RuntimeHomeFile::text(
        &request.instructions.relative_path,
        request.instructions.content.clone(),
    ));

    for capability in &request.plan.effective_capabilities {
        renderer.add_capability(capability);
    }
    renderer.add_config(&request.plan.runtime_config.model);
    renderer.add_hooks_index();
    renderer.finish()
}

#[derive(Debug)]
struct CodexHomeRenderer<'a> {
    repo_root: &'a Path,
    files: Vec<RuntimeHomeFile>,
    diagnostics: Vec<Diagnostic>,
    mcp_servers: BTreeMap<String, Value>,
    hooks: Vec<CodexHookRecord>,
}

impl<'a> CodexHomeRenderer<'a> {
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
            "runtime-setting" | "instruction" | "agent" => {}
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
        self.hooks.push(CodexHookRecord {
            id: capability.id.clone(),
            path: path_string(&target_path),
        });
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
        let parsed: Value = match toml::from_str(&encoded) {
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
        self.mcp_servers
            .insert(capability_body(&capability.id).to_string(), parsed);
    }

    fn add_config(&mut self, model: &Option<String>) {
        let mut config = toml::Table::new();
        if let Some(model) = model {
            config.insert("model".to_string(), Value::String(model.clone()));
        }
        if !self.mcp_servers.is_empty() {
            config.insert(
                "mcp_servers".to_string(),
                Value::Table(std::mem::take(&mut self.mcp_servers).into_iter().collect()),
            );
        }
        let mut encoded = match toml::to_string_pretty(&Value::Table(config)) {
            Ok(encoded) => encoded,
            Err(source) => {
                self.diagnostics.push(SUPPORT.config_render_failed(&source));
                return;
            }
        };
        if !encoded.ends_with('\n') {
            encoded.push('\n');
        }
        self.files.push(RuntimeHomeFile::text(CONFIG_FILE, encoded));
    }

    fn add_hooks_index(&mut self) {
        if self.hooks.is_empty() {
            return;
        }

        let mut encoded = serde_json::to_string_pretty(&CodexHooksJson { hooks: &self.hooks })
            .expect("hooks index serializes");
        encoded.push('\n');
        self.files.push(RuntimeHomeFile::text(HOOKS_FILE, encoded));
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

#[derive(Debug, Serialize)]
struct CodexHooksJson<'a> {
    hooks: &'a [CodexHookRecord],
}

#[derive(Debug, Serialize)]
struct CodexHookRecord {
    id: String,
    path: String,
}
