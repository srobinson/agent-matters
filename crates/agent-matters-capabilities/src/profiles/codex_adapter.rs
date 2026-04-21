//! Codex runtime home renderer.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use agent_matters_core::catalog::MANIFEST_FILE_NAME;
use agent_matters_core::domain::{Diagnostic, DiagnosticLocation, DiagnosticSeverity};
use agent_matters_core::runtime::{
    CredentialSymlinkAllowlistEntry, RuntimeHomeFile, RuntimeLaunchInstructions,
};
use serde::Serialize;
use toml::Value;

use super::adapter::{
    CODEX_RUNTIME_ID, RuntimeAdapter, RuntimeHomeRenderRequest, RuntimeHomeRenderResult,
    RuntimeLaunchRequest, launch_instructions,
};

const CONFIG_FILE: &str = "config.toml";
const HOOKS_FILE: &str = "hooks.json";
const SKILLS_DIR: &str = "skills";
const HOOKS_DIR: &str = "hooks";
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

    fn add_capability(&mut self, capability: &agent_matters_core::catalog::CapabilityIndexRecord) {
        match capability.kind.as_str() {
            "skill" => self.add_single_file_capability(capability, "source", SKILLS_DIR),
            "hook" => self.add_hook(capability),
            "mcp" => self.add_mcp_server(capability),
            "runtime-setting" | "instruction" | "agent" => {}
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
        self.hooks.push(CodexHookRecord {
            id: capability.id.clone(),
            path: path_string(&target_path),
        });
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
        let parsed: Value = match toml::from_str(&encoded) {
            Ok(parsed) => parsed,
            Err(source) => {
                self.diagnostics
                    .push(mcp_manifest_invalid(capability, source_file, &source));
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
                self.diagnostics.push(config_render_failed(&source));
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

#[derive(Debug, Serialize)]
struct CodexHooksJson<'a> {
    hooks: &'a [CodexHookRecord],
}

#[derive(Debug, Serialize)]
struct CodexHookRecord {
    id: String,
    path: String,
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
        "runtime.codex.capability-kind-unsupported",
        format!(
            "Codex adapter does not support capability kind `{}` for `{}`",
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
        "runtime.codex.file-mapping-unsupported",
        format!(
            "Codex adapter does not support `{}` file mapping `{}`; expected `{}`",
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
        "runtime.codex.file-mapping-missing",
        format!(
            "Codex adapter needs `{}` file mapping `{}`",
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
        "runtime.codex.file-read-failed",
        format!(
            "failed to read `{}` file `{}` for Codex adapter: {source}",
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
        "runtime.codex.mcp-config-invalid",
        format!(
            "failed to parse `{}` MCP file `{}` for Codex config: {source}",
            capability.id, source_file
        ),
    )
    .with_location(DiagnosticLocation::manifest_path(
        Path::new(&capability.source_path).join(source_file),
    ))
}

fn config_render_failed(source: &toml::ser::Error) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "runtime.codex.config-render-failed",
        format!("failed to render Codex config.toml: {source}"),
    )
}
