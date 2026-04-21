//! Instruction file assembly for generated runtime homes.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use agent_matters_core::catalog::ProfileIndexRecord;
use agent_matters_core::domain::{Diagnostic, DiagnosticLocation, DiagnosticSeverity};
use agent_matters_core::manifest::InstructionMarkers;
use agent_matters_core::runtime::RUNTIME_INSTRUCTIONS_FILE_NAME;
use serde::Serialize;

use crate::config::{ConfigError, load_markers, load_user_config_from_state_dir};

use super::ResolvedInstructionFragment;

const INSTRUCTION_KIND: &str = "instruction";
const AGENT_KIND: &str = "agent";
const INSTRUCTION_CONTENT_KEY: &str = "content";
const AGENT_INSTRUCTIONS_KEY: &str = "instructions";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BuildPlanInstructionOutput {
    pub markers: InstructionMarkers,
}

impl Default for BuildPlanInstructionOutput {
    fn default() -> Self {
        Self {
            markers: InstructionMarkers::HtmlComments,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AssembleProfileInstructionsRequest<'a> {
    pub repo_root: &'a Path,
    pub profile: &'a str,
    pub fragments: &'a [ResolvedInstructionFragment],
    pub output: &'a BuildPlanInstructionOutput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AssembleProfileInstructionsResult {
    pub instructions: Option<AssembledProfileInstructions>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssembledProfileInstructions {
    pub relative_path: PathBuf,
    pub content: String,
}

pub(crate) fn resolve_instruction_output(
    repo_root: &Path,
    user_state_dir: &Path,
    profile: &ProfileIndexRecord,
    diagnostics: &mut Vec<Diagnostic>,
) -> BuildPlanInstructionOutput {
    let mut output = BuildPlanInstructionOutput::default();

    match load_markers(repo_root) {
        Ok(markers) => {
            if let Some(markers) = markers
                .instructions_output
                .as_ref()
                .and_then(|defaults| defaults.markers)
            {
                output.markers = markers;
            }
        }
        Err(error) => diagnostics.push(config_load_error("repo marker defaults", &error)),
    }

    match load_user_config_from_state_dir(user_state_dir) {
        Ok(config) => {
            if let Some(markers) = config
                .instructions_output
                .as_ref()
                .and_then(|defaults| defaults.markers)
            {
                output.markers = markers;
            }
        }
        Err(error) => diagnostics.push(config_load_error("user config", &error)),
    }

    if let Some(markers) = profile.instruction_markers {
        output.markers = markers;
    }

    output
}

pub(crate) fn assemble_profile_instructions(
    request: AssembleProfileInstructionsRequest<'_>,
) -> AssembleProfileInstructionsResult {
    let mut diagnostics = Vec::new();
    let sources = instruction_sources(&request, &mut diagnostics);
    if has_error_diagnostics(&diagnostics) {
        return AssembleProfileInstructionsResult {
            instructions: None,
            diagnostics,
        };
    }

    AssembleProfileInstructionsResult {
        instructions: Some(AssembledProfileInstructions {
            relative_path: PathBuf::from(RUNTIME_INSTRUCTIONS_FILE_NAME),
            content: render_instructions(request.profile, &sources, request.output.markers),
        }),
        diagnostics,
    }
}

fn instruction_sources(
    request: &AssembleProfileInstructionsRequest<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<InstructionSource> {
    let mut sources = Vec::new();
    for fragment in request.fragments {
        let Some(relative_path) = fragment_instruction_path(fragment, diagnostics) else {
            continue;
        };
        let absolute_path = request.repo_root.join(&relative_path);
        match fs::read_to_string(&absolute_path) {
            Ok(content) => sources.push(InstructionSource {
                id: fragment.id.clone(),
                relative_path: stable_path(&relative_path),
                content,
            }),
            Err(source) => diagnostics.push(instruction_read_failed(&relative_path, &source)),
        }
    }
    sources
}

fn fragment_instruction_path(
    fragment: &ResolvedInstructionFragment,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<PathBuf> {
    let file_key = match fragment.kind.as_str() {
        INSTRUCTION_KIND => INSTRUCTION_CONTENT_KEY,
        AGENT_KIND => AGENT_INSTRUCTIONS_KEY,
        _ => {
            diagnostics.push(unsupported_instruction_kind(fragment));
            return None;
        }
    };

    match fragment.files.get(file_key) {
        Some(file_name) => Some(Path::new(&fragment.source_path).join(file_name)),
        None => {
            diagnostics.push(missing_instruction_file_reference(fragment, file_key));
            None
        }
    }
}

fn render_instructions(
    profile: &str,
    sources: &[InstructionSource],
    markers: InstructionMarkers,
) -> String {
    let mut output = String::new();

    if markers == InstructionMarkers::TopNotice {
        push_section(&mut output, &top_notice(profile, sources));
    }

    for source in sources {
        let section = match markers {
            InstructionMarkers::HtmlComments => html_marked_fragment(source),
            InstructionMarkers::TopNotice | InstructionMarkers::None => {
                normalized_fragment_content(&source.content)
            }
        };
        push_section(&mut output, &section);
    }

    if !output.is_empty() {
        output.push('\n');
    }
    output
}

fn push_section(output: &mut String, section: &str) {
    if !output.is_empty() {
        output.push_str("\n\n");
    }
    output.push_str(section.trim_end_matches('\n'));
}

fn html_marked_fragment(source: &InstructionSource) -> String {
    format!(
        "<!-- agent-matters:begin id=\"{}\" source=\"{}\" -->\n{}\n<!-- agent-matters:end id=\"{}\" -->",
        source.id,
        source.relative_path,
        normalized_fragment_content(&source.content),
        source.id
    )
}

fn top_notice(profile: &str, sources: &[InstructionSource]) -> String {
    let source_list = sources
        .iter()
        .map(|source| format!("{} ({})", source.id, source.relative_path))
        .collect::<Vec<_>>()
        .join("; ");
    format!(
        "<!-- Generated by agent-matters for profile `{profile}`. Source fragments: {source_list}. -->"
    )
}

fn normalized_fragment_content(content: &str) -> String {
    content
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim_end_matches('\n')
        .to_string()
}

fn has_error_diagnostics(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
}

fn stable_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn config_load_error(label: &str, error: &ConfigError) -> Diagnostic {
    match error {
        ConfigError::Io { path, source } => Diagnostic::new(
            DiagnosticSeverity::Error,
            "profile.instructions-output-config-read-failed",
            format!("failed to read {label} `{}`: {source}", path.display()),
        )
        .with_location(DiagnosticLocation::manifest_path(path)),
        ConfigError::Parse { path, source } => Diagnostic::new(
            DiagnosticSeverity::Error,
            "profile.instructions-output-config-parse-failed",
            format!("failed to parse {label} `{}`: {source}", path.display()),
        )
        .with_location(DiagnosticLocation::manifest_path(path)),
    }
}

fn instruction_read_failed(path: &Path, source: &io::Error) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "profile.instructions.read-failed",
        format!(
            "failed to read instruction fragment `{}`: {source}",
            path.display()
        ),
    )
    .with_location(DiagnosticLocation::manifest_path(path))
}

fn unsupported_instruction_kind(fragment: &ResolvedInstructionFragment) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "profile.instructions.unsupported-kind",
        format!(
            "profile instruction `{}` has unsupported capability kind `{}`",
            fragment.id, fragment.kind
        ),
    )
    .with_location(DiagnosticLocation::manifest_path(PathBuf::from(
        &fragment.source_path,
    )))
    .with_recovery_hint("use instruction or agent capabilities in profile instructions")
}

fn missing_instruction_file_reference(
    fragment: &ResolvedInstructionFragment,
    file_key: &str,
) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "profile.instructions.file-reference-missing",
        format!(
            "profile instruction `{}` is missing `[files].{file_key}`",
            fragment.id
        ),
    )
    .with_location(DiagnosticLocation::manifest_path(PathBuf::from(
        &fragment.source_path,
    )))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InstructionSource {
    id: String,
    relative_path: String,
    content: String,
}
