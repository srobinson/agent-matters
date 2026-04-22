use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use agent_matters_core::catalog::{
    CATALOG_DIR_NAME, MANIFEST_FILE_NAME, PROFILES_DIR_NAME, VENDOR_DIR_NAME,
    capability_kind_dir_name,
};
use agent_matters_core::domain::{
    CapabilityId, CapabilityKind, Diagnostic, DiagnosticLocation, DiagnosticSeverity,
    EnvVarRequirement, ProfileId, ProfileKind, Provenance, Requirements, RuntimeId,
};
use agent_matters_core::manifest::{
    CapabilityFilesManifest, CapabilityManifest, CapabilityRuntimeManifest,
    CapabilityRuntimesManifest, InstructionMarkers, InstructionsOutputManifest, ProfileManifest,
    ProfileRuntimeManifest, ProfileRuntimesManifest,
};
use serde_json::{Value as JsonValue, json};
use toml::Value as TomlValue;

use super::sanitize::{id_segment, sanitize_json_value, sanitize_toml_value, should_skip_file};
use super::{
    ImportRuntimeHomeRequest, PlannedCapability, PlannedFile, RuntimeHomeImportSkippedFile,
    RuntimeImportPlan,
};

const LOCAL_RUNTIME_IMPORT_SOURCE: &str = "local-runtime-import";
const CODEX_RUNTIME: &str = "codex";
const CLAUDE_RUNTIME: &str = "claude";
const FALLBACK_RUNTIME: &str = "runtime";

pub(super) fn inspect_runtime_home(request: &ImportRuntimeHomeRequest) -> RuntimeImportPlan {
    let runtime = resolve_runtime(request);
    let profile = resolve_profile(request, &runtime.value);
    let profile_id = ProfileId::new(profile.clone());
    let runtime_id = RuntimeId::new(runtime.value.clone());
    let profile_manifest_path = profile_id
        .as_ref()
        .map(|id| {
            PathBuf::from(PROFILES_DIR_NAME)
                .join(id.as_str())
                .join(MANIFEST_FILE_NAME)
        })
        .unwrap_or_else(|_| PathBuf::from(PROFILES_DIR_NAME).join(MANIFEST_FILE_NAME));

    let mut builder = RuntimeImportBuilder {
        request,
        runtime_id,
        runtime_diagnostics: runtime.diagnostics,
        profile_id,
        profile,
        capabilities: Vec::new(),
        instructions: Vec::new(),
        skipped_files: Vec::new(),
        diagnostics: Vec::new(),
        model: None,
    };
    builder.validate_inputs();
    if !has_error_diagnostics(&builder.diagnostics) {
        builder.inspect();
    }
    builder.finish(profile_manifest_path)
}

struct RuntimeImportBuilder<'a> {
    request: &'a ImportRuntimeHomeRequest,
    runtime_id: Result<RuntimeId, agent_matters_core::domain::IdError>,
    runtime_diagnostics: Vec<Diagnostic>,
    profile_id: Result<ProfileId, agent_matters_core::domain::IdError>,
    profile: String,
    capabilities: Vec<PlannedCapability>,
    instructions: Vec<CapabilityId>,
    skipped_files: Vec<RuntimeHomeImportSkippedFile>,
    diagnostics: Vec<Diagnostic>,
    model: Option<String>,
}

struct CapabilityInput {
    id: CapabilityId,
    kind: CapabilityKind,
    summary: String,
    role: String,
    file_name: String,
    source_path: String,
    contents: Vec<u8>,
}

impl RuntimeImportBuilder<'_> {
    fn validate_inputs(&mut self) {
        self.diagnostics.append(&mut self.runtime_diagnostics);
        if self.runtime_id.is_err()
            || !matches!(self.runtime_value(), CODEX_RUNTIME | CLAUDE_RUNTIME)
        {
            self.diagnostics.push(Diagnostic::new(
                DiagnosticSeverity::Error,
                "source.runtime-import-unsupported-runtime",
                format!(
                    "runtime import supports `codex` and `claude`, got `{}`",
                    self.runtime_value()
                ),
            ));
        }
        if self.profile_id.is_err() {
            self.diagnostics.push(Diagnostic::new(
                DiagnosticSeverity::Error,
                "source.runtime-import-profile-invalid",
                format!(
                    "runtime import profile id `{}` is not a path safe id",
                    self.profile
                ),
            ));
        }
        match fs::metadata(&self.request.source_home) {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => self.diagnostics.push(Diagnostic::new(
                DiagnosticSeverity::Error,
                "source.runtime-import-home-not-directory",
                format!(
                    "runtime import source `{}` is not a directory",
                    self.request.source_home.display()
                ),
            )),
            Err(source) => self.diagnostics.push(
                Diagnostic::new(
                    DiagnosticSeverity::Error,
                    "source.runtime-import-home-missing",
                    format!(
                        "failed to read runtime import source `{}`: {source}",
                        self.request.source_home.display()
                    ),
                )
                .with_location(DiagnosticLocation::source_path(&self.request.source_home)),
            ),
        }
    }

    fn inspect(&mut self) {
        match self.runtime_value() {
            CODEX_RUNTIME => self.inspect_codex(),
            CLAUDE_RUNTIME => self.inspect_claude(),
            _ => {}
        }
        if self.capabilities.is_empty() && self.instructions.is_empty() {
            self.diagnostics.push(Diagnostic::new(
                DiagnosticSeverity::Warning,
                "source.runtime-import-empty",
                "runtime import found no supported non-secret artifacts",
            ));
        }
    }

    fn runtime_value(&self) -> &str {
        self.runtime_id
            .as_ref()
            .map(RuntimeId::as_str)
            .unwrap_or(FALLBACK_RUNTIME)
    }

    fn inspect_codex(&mut self) {
        self.add_instruction_if_present("AGENTS.md");
        self.add_skipped_if_present("auth.json");
        self.add_skills();
        self.add_hooks();
        self.add_codex_config();
    }

    fn inspect_claude(&mut self) {
        self.add_instruction_if_present("CLAUDE.md");
        self.add_skipped_if_present(".credentials.json");
        self.add_skills();
        self.add_agents();
        self.add_hooks();
        self.add_claude_settings();
        self.add_claude_mcp_config();
    }

    fn add_instruction_if_present(&mut self, file_name: &str) {
        let source = self.request.source_home.join(file_name);
        let Ok(contents) = fs::read(&source) else {
            return;
        };
        let profile = self.profile_id.as_ref().expect("validated profile id");
        let body = format!("{}-instructions", profile.as_str());
        let id = CapabilityId::new(CapabilityKind::Instruction, body).expect("safe instruction id");
        self.instructions.push(id.clone());
        self.capabilities.push(self.capability(CapabilityInput {
            id,
            kind: CapabilityKind::Instruction,
            summary: "Imported runtime instructions.".to_string(),
            role: "content".to_string(),
            file_name: file_name.to_string(),
            source_path: file_name.to_string(),
            contents,
        }));
    }

    fn add_skipped_if_present(&mut self, relative_path: &str) {
        let path = PathBuf::from(relative_path);
        if self.request.source_home.join(&path).exists() {
            self.skipped_files.push(RuntimeHomeImportSkippedFile {
                path,
                reason: "credential file is not copied into catalog content".to_string(),
            });
        }
    }

    fn add_skills(&mut self) {
        for file in self.files_under("skills") {
            let Some(skill_dir) = file.components().next() else {
                continue;
            };
            let body = id_segment(&skill_dir.as_os_str().to_string_lossy());
            let id = CapabilityId::new(CapabilityKind::Skill, body).expect("safe skill id");
            self.add_file_capability(
                id,
                CapabilityKind::Skill,
                "Imported runtime skill.",
                "source",
                Path::new("skills").join(&file),
                file,
            );
        }
    }

    fn add_agents(&mut self) {
        for file in self.files_under("agents") {
            let Some(stem) = file.file_stem() else {
                continue;
            };
            let body = id_segment(&stem.to_string_lossy());
            let id = CapabilityId::new(CapabilityKind::Agent, body).expect("safe agent id");
            self.add_file_capability(
                id.clone(),
                CapabilityKind::Agent,
                "Imported Claude agent.",
                "instructions",
                Path::new("agents").join(&file),
                file,
            );
            self.instructions.push(id);
        }
    }

    fn add_hooks(&mut self) {
        for file in self.files_under("hooks") {
            let Some(stem) = file.file_stem() else {
                continue;
            };
            let body = id_segment(&stem.to_string_lossy());
            let id = CapabilityId::new(CapabilityKind::Hook, body).expect("safe hook id");
            self.add_file_capability(
                id,
                CapabilityKind::Hook,
                "Imported runtime hook.",
                "script",
                Path::new("hooks").join(&file),
                file,
            );
        }
    }

    fn add_codex_config(&mut self) {
        let path = self.request.source_home.join("config.toml");
        let Ok(raw) = fs::read_to_string(&path) else {
            return;
        };
        let parsed = match raw.parse::<TomlValue>() {
            Ok(parsed) => parsed,
            Err(source) => {
                self.config_invalid("config.toml", &source.to_string());
                return;
            }
        };
        self.model = parsed
            .get("model")
            .and_then(TomlValue::as_str)
            .map(ToOwned::to_owned);
        if let Some(TomlValue::Table(servers)) = parsed.get("mcp_servers") {
            for (name, server) in servers {
                let mut server = server.clone();
                self.add_mcp_from_toml(name, &mut server, "config.toml");
            }
        }
    }

    fn add_claude_settings(&mut self) {
        let path = self.request.source_home.join("settings.json");
        let Ok(raw) = fs::read_to_string(&path) else {
            return;
        };
        match serde_json::from_str::<JsonValue>(&raw) {
            Ok(JsonValue::Object(settings)) => {
                self.model = settings
                    .get("model")
                    .and_then(JsonValue::as_str)
                    .map(ToOwned::to_owned);
            }
            Ok(_) => self.config_invalid("settings.json", "root JSON value must be an object"),
            Err(source) => self.config_invalid("settings.json", &source.to_string()),
        }
    }

    fn add_claude_mcp_config(&mut self) {
        let path = self.request.source_home.join(".claude.json");
        let Ok(raw) = fs::read_to_string(&path) else {
            return;
        };
        let parsed = match serde_json::from_str::<JsonValue>(&raw) {
            Ok(parsed) => parsed,
            Err(source) => {
                self.config_invalid(".claude.json", &source.to_string());
                return;
            }
        };
        let Some(servers) = parsed.get("mcpServers").and_then(JsonValue::as_object) else {
            return;
        };
        for (name, server) in servers {
            let mut server = server.clone();
            self.add_mcp_from_json(name, &mut server, ".claude.json");
        }
    }

    fn add_mcp_from_toml(&mut self, name: &str, server: &mut TomlValue, source_path: &str) {
        let mut env_keys = Vec::new();
        if sanitize_toml_value(server, &mut env_keys) {
            self.diagnostics.push(sanitized_config(source_path, name));
        }
        let encoded = match toml::to_string_pretty(server) {
            Ok(mut encoded) => {
                if !encoded.ends_with('\n') {
                    encoded.push('\n');
                }
                encoded
            }
            Err(source) => {
                self.config_invalid(source_path, &source.to_string());
                return;
            }
        };
        self.add_mcp(name, source_path, encoded.into_bytes(), env_keys);
    }

    fn add_mcp_from_json(&mut self, name: &str, server: &mut JsonValue, source_path: &str) {
        let mut env_keys = Vec::new();
        if sanitize_json_value(server, &mut env_keys) {
            self.diagnostics.push(sanitized_config(source_path, name));
        }
        let Some(toml_value) = json_to_toml(server) else {
            self.config_invalid(
                source_path,
                "MCP server contains a value TOML cannot represent",
            );
            return;
        };
        let encoded = match toml::to_string_pretty(&toml_value) {
            Ok(mut encoded) => {
                if !encoded.ends_with('\n') {
                    encoded.push('\n');
                }
                encoded
            }
            Err(source) => {
                self.config_invalid(source_path, &source.to_string());
                return;
            }
        };
        self.add_mcp(name, source_path, encoded.into_bytes(), env_keys);
    }

    fn add_mcp(&mut self, name: &str, source_path: &str, contents: Vec<u8>, env_keys: Vec<String>) {
        let id = CapabilityId::new(CapabilityKind::Mcp, id_segment(name)).expect("safe mcp id");
        let mut capability = self.capability(CapabilityInput {
            id,
            kind: CapabilityKind::Mcp,
            summary: "Imported runtime MCP server.".to_string(),
            role: "manifest".to_string(),
            file_name: "server.toml".to_string(),
            source_path: format!("{source_path}#{name}"),
            contents,
        });
        let env = env_requirements(env_keys, &mut self.diagnostics);
        if !env.is_empty() {
            capability.manifest.requires = Some(Requirements {
                capabilities: Vec::new(),
                env,
            });
        }
        self.capabilities.push(capability);
    }

    fn add_file_capability(
        &mut self,
        id: CapabilityId,
        kind: CapabilityKind,
        summary: &str,
        role: &str,
        source_relative: PathBuf,
        catalog_relative: PathBuf,
    ) {
        let source = self.request.source_home.join(&source_relative);
        let Some(reason) = should_skip_file(&source_relative) else {
            match fs::read(&source) {
                Ok(contents) => {
                    let file_name = stable_path(&catalog_relative);
                    self.capabilities.push(self.capability(CapabilityInput {
                        id,
                        kind,
                        summary: summary.to_string(),
                        role: role.to_string(),
                        file_name,
                        source_path: stable_path(&source_relative),
                        contents,
                    }));
                }
                Err(source) => self
                    .diagnostics
                    .push(read_failed(&source_relative, &source)),
            }
            return;
        };
        self.skipped_files.push(RuntimeHomeImportSkippedFile {
            path: source_relative,
            reason: reason.to_string(),
        });
    }

    fn capability(&self, input: CapabilityInput) -> PlannedCapability {
        let runtime = self.runtime_id.as_ref().expect("validated runtime id");
        let body = input.id.body().to_string();
        let locator = format!("{}/{}/{}", runtime.as_str(), input.kind.as_str(), body);
        let manifest_path = PathBuf::from(CATALOG_DIR_NAME)
            .join(capability_kind_dir_name(input.kind))
            .join(&body)
            .join(MANIFEST_FILE_NAME);
        let vendor_path = PathBuf::from(VENDOR_DIR_NAME)
            .join(LOCAL_RUNTIME_IMPORT_SOURCE)
            .join(&locator);
        PlannedCapability {
            manifest: CapabilityManifest {
                id: input.id,
                kind: input.kind,
                summary: input.summary,
                files: CapabilityFilesManifest {
                    entries: BTreeMap::from([(input.role, input.file_name.clone())]),
                },
                runtimes: runtime_support(runtime),
                requires: None,
                origin: Some(Provenance::external(
                    LOCAL_RUNTIME_IMPORT_SOURCE,
                    locator,
                    None,
                )),
            },
            source_path: input.source_path.clone(),
            manifest_path,
            vendor_path,
            catalog_files: vec![PlannedFile {
                relative_path: PathBuf::from(input.file_name),
                contents: input.contents,
            }],
            vendor_record: json!({
                "source": LOCAL_RUNTIME_IMPORT_SOURCE,
                "runtime": runtime.as_str(),
                "capability_id": body,
                "source_path": input.source_path,
            }),
        }
    }

    fn files_under(&mut self, dir: &str) -> Vec<PathBuf> {
        let root = self.request.source_home.join(dir);
        let mut files = Vec::new();
        collect_files(
            &root,
            &root,
            &mut files,
            &mut self.skipped_files,
            &mut self.diagnostics,
        );
        files
    }

    fn config_invalid(&mut self, path: &str, source: &str) {
        self.diagnostics.push(Diagnostic::new(
            DiagnosticSeverity::Error,
            "source.runtime-import-config-invalid",
            format!("failed to parse runtime import config `{path}`: {source}"),
        ));
    }

    fn finish(self, profile_manifest_path: PathBuf) -> RuntimeImportPlan {
        let runtime = self
            .runtime_id
            .unwrap_or_else(|_| RuntimeId::new(FALLBACK_RUNTIME).expect("fallback runtime id"));
        let profile_id = self
            .profile_id
            .unwrap_or_else(|_| ProfileId::new("runtime-import").expect("fallback profile id"));
        let profile = ProfileManifest {
            id: profile_id.clone(),
            kind: ProfileKind::Persona,
            summary: format!("Imported {} runtime home.", runtime.as_str()),
            capabilities: self
                .capabilities
                .iter()
                .filter(|capability| !self.instructions.contains(&capability.manifest.id))
                .map(|capability| capability.manifest.id.clone())
                .collect(),
            instructions: self.instructions,
            scope: None,
            runtimes: Some(ProfileRuntimesManifest {
                default: Some(runtime.clone()),
                entries: BTreeMap::from([(
                    runtime.clone(),
                    ProfileRuntimeManifest {
                        enabled: true,
                        model: self.model,
                    },
                )]),
            }),
            instructions_output: Some(InstructionsOutputManifest {
                markers: Some(InstructionMarkers::None),
            }),
        };
        RuntimeImportPlan {
            runtime: runtime.as_str().to_string(),
            source_home: self.request.source_home.clone(),
            profile_id: profile_id.as_str().to_string(),
            profile_manifest_path,
            profile,
            capabilities: self.capabilities,
            skipped_files: self.skipped_files,
            diagnostics: self.diagnostics,
        }
    }
}

struct RuntimeSelection {
    value: String,
    diagnostics: Vec<Diagnostic>,
}

fn resolve_runtime(request: &ImportRuntimeHomeRequest) -> RuntimeSelection {
    if let Some(runtime) = &request.runtime {
        return RuntimeSelection {
            value: runtime.clone(),
            diagnostics: Vec::new(),
        };
    }

    match infer_runtime_from_path(&request.source_home) {
        RuntimeInference::Detected(runtime) => RuntimeSelection {
            value: runtime.to_string(),
            diagnostics: Vec::new(),
        },
        RuntimeInference::Ambiguous => RuntimeSelection {
            value: FALLBACK_RUNTIME.to_string(),
            diagnostics: vec![
                Diagnostic::new(
                    DiagnosticSeverity::Error,
                    "source.runtime-import-runtime-ambiguous",
                    format!(
                        "runtime import source `{}` looks like both codex and claude",
                        request.source_home.display()
                    ),
                )
                .with_recovery_hint("pass `--runtime codex` or `--runtime claude`"),
            ],
        },
        RuntimeInference::Undetected => RuntimeSelection {
            value: FALLBACK_RUNTIME.to_string(),
            diagnostics: vec![
                Diagnostic::new(
                    DiagnosticSeverity::Error,
                    "source.runtime-import-runtime-undetected",
                    format!(
                        "runtime import could not infer codex or claude from `{}`",
                        request.source_home.display()
                    ),
                )
                .with_recovery_hint("pass `--runtime codex` or `--runtime claude`"),
            ],
        },
    }
}

fn resolve_profile(request: &ImportRuntimeHomeRequest, runtime: &str) -> String {
    if let Some(profile) = &request.profile {
        return profile.clone();
    }

    let base = request
        .source_home
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.trim_start_matches('.'))
        .filter(|name| !name.is_empty())
        .unwrap_or(runtime);
    format!("imported-{}", id_segment(base))
}

enum RuntimeInference {
    Detected(&'static str),
    Ambiguous,
    Undetected,
}

fn infer_runtime_from_path(source_home: &Path) -> RuntimeInference {
    match source_home
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.trim_start_matches('.'))
    {
        Some(CODEX_RUNTIME) => return RuntimeInference::Detected(CODEX_RUNTIME),
        Some(CLAUDE_RUNTIME) => return RuntimeInference::Detected(CLAUDE_RUNTIME),
        _ => {}
    }

    let codex_markers = ["AGENTS.md", "config.toml", "auth.json"];
    let claude_markers = [
        "CLAUDE.md",
        "settings.json",
        ".claude.json",
        ".credentials.json",
        "agents",
    ];
    let codex = codex_markers
        .iter()
        .any(|marker| source_home.join(marker).exists());
    let claude = claude_markers
        .iter()
        .any(|marker| source_home.join(marker).exists());

    match (codex, claude) {
        (true, false) => RuntimeInference::Detected(CODEX_RUNTIME),
        (false, true) => RuntimeInference::Detected(CLAUDE_RUNTIME),
        (true, true) => RuntimeInference::Ambiguous,
        (false, false) => RuntimeInference::Undetected,
    }
}

fn runtime_support(runtime: &RuntimeId) -> CapabilityRuntimesManifest {
    CapabilityRuntimesManifest {
        entries: BTreeMap::from([(
            runtime.clone(),
            CapabilityRuntimeManifest { supported: true },
        )]),
    }
}

fn collect_files(
    root: &Path,
    current: &Path,
    files: &mut Vec<PathBuf>,
    skipped: &mut Vec<RuntimeHomeImportSkippedFile>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let entries = match fs::read_dir(current) {
        Ok(entries) => entries,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return,
        Err(source) => {
            diagnostics.push(read_failed(current, &source));
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(source) => {
                diagnostics.push(read_failed(current, &source));
                continue;
            }
        };
        let path = entry.path();
        let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(source) => {
                diagnostics.push(read_failed(&relative, &source));
                continue;
            }
        };
        if file_type.is_symlink() {
            skipped.push(RuntimeHomeImportSkippedFile {
                path: relative,
                reason: "symlink is not copied during runtime import".to_string(),
            });
        } else if file_type.is_dir() {
            collect_files(root, &path, files, skipped, diagnostics);
        } else if file_type.is_file() {
            files.push(relative);
        }
    }
}

fn json_to_toml(value: &JsonValue) -> Option<TomlValue> {
    match value {
        JsonValue::Null => None,
        JsonValue::Bool(value) => Some(TomlValue::Boolean(*value)),
        JsonValue::Number(value) => value
            .as_i64()
            .map(TomlValue::Integer)
            .or_else(|| value.as_f64().map(TomlValue::Float)),
        JsonValue::String(value) => Some(TomlValue::String(value.clone())),
        JsonValue::Array(values) => values
            .iter()
            .map(json_to_toml)
            .collect::<Option<Vec<_>>>()
            .map(TomlValue::Array),
        JsonValue::Object(values) => {
            let mut table = toml::Table::new();
            for (key, value) in values {
                table.insert(key.clone(), json_to_toml(value)?);
            }
            Some(TomlValue::Table(table))
        }
    }
}

fn env_requirements(
    env_keys: Vec<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<EnvVarRequirement> {
    let mut unique = BTreeSet::new();
    let mut requirements = Vec::new();
    for key in env_keys {
        if !unique.insert(key.clone()) {
            continue;
        }
        match EnvVarRequirement::new(&key) {
            Ok(requirement) => requirements.push(requirement),
            Err(source) => diagnostics.push(Diagnostic::new(
                DiagnosticSeverity::Warning,
                "source.runtime-import-env-invalid",
                format!("skipped invalid imported env requirement `{key}`: {source}"),
            )),
        }
    }
    requirements
}

fn sanitized_config(path: &str, server: &str) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Warning,
        "source.runtime-import-secret-redacted",
        format!("sanitized secret-like values from `{path}` MCP server `{server}`"),
    )
}

fn read_failed(path: impl AsRef<Path>, source: &std::io::Error) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "source.runtime-import-read-failed",
        format!(
            "failed to read runtime import path `{}`: {source}",
            path.as_ref().display()
        ),
    )
    .with_location(DiagnosticLocation::source_path(path.as_ref()))
}

fn stable_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn has_error_diagnostics(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
}
