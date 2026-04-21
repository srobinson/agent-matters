//! Resolve profile manifests into compiler ready inventory.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use agent_matters_core::catalog::{CapabilityIndexRecord, MANIFEST_FILE_NAME, ProfileIndexRecord};
use agent_matters_core::domain::{Diagnostic, DiagnosticLocation, DiagnosticSeverity};
use serde::Serialize;

use crate::catalog::{CatalogIndexError, LoadCatalogIndexRequest, load_or_refresh_catalog_index};
use crate::profiles::runtime::{ResolvedRuntimeConfig, resolve_runtime_configs};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveProfileRequest {
    pub repo_root: PathBuf,
    pub user_state_dir: PathBuf,
    pub profile: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolveProfileResult {
    pub profile: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record: Option<ProfileIndexRecord>,
    pub effective_capabilities: Vec<CapabilityIndexRecord>,
    pub instruction_fragments: Vec<ResolvedInstructionFragment>,
    pub runtime_configs: Vec<ResolvedRuntimeConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_runtime: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
}

impl ResolveProfileResult {
    pub fn has_error_diagnostics(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedInstructionFragment {
    pub id: String,
    pub kind: String,
    pub source_path: String,
    pub files: BTreeMap<String, String>,
}

pub fn resolve_profile(
    request: ResolveProfileRequest,
) -> Result<ResolveProfileResult, CatalogIndexError> {
    let repo_root = request.repo_root;
    let user_state_dir = request.user_state_dir;
    let loaded = load_or_refresh_catalog_index(LoadCatalogIndexRequest {
        repo_root: repo_root.clone(),
        user_state_dir: user_state_dir.clone(),
    })?;
    let record = loaded.index.profile(&request.profile).cloned();
    let mut result = ResolveProfileResult {
        profile: request.profile,
        record,
        effective_capabilities: Vec::new(),
        instruction_fragments: Vec::new(),
        runtime_configs: Vec::new(),
        selected_runtime: None,
        diagnostics: loaded.diagnostics,
    };

    let Some(profile) = result.record.as_ref() else {
        result.diagnostics.push(profile_not_found(&result.profile));
        return Ok(result);
    };

    let profile_manifest_path = PathBuf::from(&profile.source_path).join(MANIFEST_FILE_NAME);
    let resolution = resolve_profile_ids(profile, &profile_manifest_path, &mut result.diagnostics);
    let mut missing_references = BTreeSet::new();

    for reference in resolution.effective_capabilities {
        match loaded.index.capability(&reference.id) {
            Some(record) => result.effective_capabilities.push(record.clone()),
            None if missing_references.insert(reference.id.clone()) => {
                result
                    .diagnostics
                    .push(missing_capability(&reference, &profile_manifest_path));
            }
            None => {}
        }
    }

    for reference in resolution.instruction_fragments {
        match loaded.index.capability(&reference.id) {
            Some(record) => {
                result
                    .instruction_fragments
                    .push(instruction_fragment(record));
            }
            None if missing_references.insert(reference.id.clone()) => {
                result
                    .diagnostics
                    .push(missing_capability(&reference, &profile_manifest_path));
            }
            None => {}
        }
    }

    let runtime_resolution = resolve_runtime_configs(
        &repo_root,
        &user_state_dir,
        profile,
        &result.effective_capabilities,
        &profile_manifest_path,
    );
    result.runtime_configs = runtime_resolution.runtime_configs;
    result.selected_runtime = runtime_resolution.selected_runtime;
    result.diagnostics.extend(runtime_resolution.diagnostics);

    Ok(result)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProfileReference {
    id: String,
    field: &'static str,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct ProfileResolution {
    effective_capabilities: Vec<ProfileReference>,
    instruction_fragments: Vec<ProfileReference>,
}

fn resolve_profile_ids(
    profile: &ProfileIndexRecord,
    profile_manifest_path: &Path,
    diagnostics: &mut Vec<Diagnostic>,
) -> ProfileResolution {
    let mut resolution = ProfileResolution::default();
    let mut seen_effective = BTreeSet::new();
    let mut seen_instructions = BTreeSet::new();

    for id in &profile.capabilities {
        if seen_effective.insert(id.clone()) {
            resolution
                .effective_capabilities
                .push(ProfileReference::capability(id));
        } else {
            diagnostics.push(duplicate_capability_reference(id, profile_manifest_path));
        }
    }

    for id in &profile.instructions {
        if seen_instructions.insert(id.clone()) {
            resolution
                .instruction_fragments
                .push(ProfileReference::instruction(id));
        } else {
            diagnostics.push(duplicate_instruction_reference(id, profile_manifest_path));
            continue;
        }

        if seen_effective.insert(id.clone()) {
            resolution
                .effective_capabilities
                .push(ProfileReference::instruction(id));
        }
    }

    resolution
}

impl ProfileReference {
    fn capability(id: &str) -> Self {
        Self {
            id: id.to_string(),
            field: "capabilities",
        }
    }

    fn instruction(id: &str) -> Self {
        Self {
            id: id.to_string(),
            field: "instructions",
        }
    }
}

fn instruction_fragment(record: &CapabilityIndexRecord) -> ResolvedInstructionFragment {
    ResolvedInstructionFragment {
        id: record.id.clone(),
        kind: record.kind.clone(),
        source_path: record.source_path.clone(),
        files: record.files.clone(),
    }
}

fn profile_not_found(profile: &str) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "profile.resolve-not-found",
        format!("profile `{profile}` was not found in the catalog index"),
    )
    .with_location(DiagnosticLocation::field("profile"))
    .with_recovery_hint("run `agent-matters profiles list` to inspect exact profile ids")
}

fn missing_capability(reference: &ProfileReference, profile_manifest_path: &Path) -> Diagnostic {
    let code = if reference.field == "instructions" {
        "profile.instruction-not-found"
    } else {
        "profile.capability-not-found"
    };

    Diagnostic::new(
        DiagnosticSeverity::Error,
        code,
        format!(
            "profile references missing capability `{}` in `{}`",
            reference.id, reference.field
        ),
    )
    .with_location(DiagnosticLocation::manifest_field(
        profile_manifest_path,
        reference.field,
    ))
    .with_recovery_hint("run `agent-matters capabilities list` to inspect exact capability ids")
}

fn duplicate_capability_reference(id: &str, profile_manifest_path: &Path) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Warning,
        "profile.duplicate-capability-reference",
        format!("profile repeats capability `{id}` in `capabilities`"),
    )
    .with_location(DiagnosticLocation::manifest_field(
        profile_manifest_path,
        "capabilities",
    ))
    .with_recovery_hint("remove repeated entries from the profile capability list")
}

fn duplicate_instruction_reference(id: &str, profile_manifest_path: &Path) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "profile.duplicate-instruction-reference",
        format!("profile repeats instruction fragment `{id}` in `instructions`"),
    )
    .with_location(DiagnosticLocation::manifest_field(
        profile_manifest_path,
        "instructions",
    ))
    .with_recovery_hint("remove repeated entries from the profile instruction list")
}
