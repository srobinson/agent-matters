//! Build plan generation for resolved profiles.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use agent_matters_core::catalog::{CapabilityIndexRecord, MANIFEST_FILE_NAME, ProfileIndexRecord};
use agent_matters_core::config::{
    REPO_DEFAULTS_DIR_NAME, RUNTIMES_FILE_NAME, USER_CONFIG_FILE_NAME,
};
use agent_matters_core::domain::{Diagnostic, DiagnosticLocation, DiagnosticSeverity};
use agent_matters_core::runtime::{
    BUILD_PLAN_SCHEMA_VERSION, FingerprintBuilder, runtime_build_dir, runtime_home_dir,
    runtime_pointer_dir,
};
use serde::Serialize;

use crate::catalog::CatalogIndexError;

use super::{
    ResolveProfileRequest, ResolvedInstructionFragment, ResolvedRuntimeConfig, resolve_profile,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildProfilePlanRequest {
    pub repo_root: PathBuf,
    pub user_state_dir: PathBuf,
    pub profile: String,
    pub runtime: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BuildProfilePlanResult {
    pub profile: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<ProfileBuildPlan>,
    pub diagnostics: Vec<Diagnostic>,
}

impl BuildProfilePlanResult {
    pub fn has_error_diagnostics(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProfileBuildPlan {
    pub schema_version: u16,
    pub profile: String,
    pub runtime: String,
    pub adapter_version: String,
    pub fingerprint: String,
    pub build_id: String,
    pub paths: BuildPlanPaths,
    pub profile_record: ProfileIndexRecord,
    pub effective_capabilities: Vec<CapabilityIndexRecord>,
    pub instruction_fragments: Vec<ResolvedInstructionFragment>,
    pub runtime_config: ResolvedRuntimeConfig,
    pub content_inputs: Vec<BuildPlanContentInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BuildPlanPaths {
    pub build_dir: PathBuf,
    pub home_dir: PathBuf,
    pub runtime_pointer: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BuildPlanContentInput {
    pub role: String,
    pub path: String,
    pub content_fingerprint: String,
}

pub fn plan_profile_build(
    request: BuildProfilePlanRequest,
) -> Result<BuildProfilePlanResult, CatalogIndexError> {
    let resolved = resolve_profile(ResolveProfileRequest {
        repo_root: request.repo_root.clone(),
        user_state_dir: request.user_state_dir.clone(),
        profile: request.profile.clone(),
    })?;
    let mut diagnostics = resolved.diagnostics;
    if request.runtime.is_some() {
        remove_default_runtime_diagnostics(&mut diagnostics);
    }
    let mut result = BuildProfilePlanResult {
        profile: request.profile,
        plan: None,
        diagnostics: Vec::new(),
    };

    let Some(profile_record) = resolved.record else {
        result.diagnostics = diagnostics;
        return Ok(result);
    };

    let runtime_config = select_runtime_config(
        request.runtime.as_deref(),
        resolved.selected_runtime.as_deref(),
        &resolved.runtime_configs,
        &profile_record,
        &mut diagnostics,
    );
    if has_error_diagnostics(&diagnostics) {
        result.diagnostics = diagnostics;
        return Ok(result);
    }
    let Some(runtime_config) = runtime_config else {
        result.diagnostics = diagnostics;
        return Ok(result);
    };

    let candidates = content_input_candidates(
        &request.repo_root,
        &request.user_state_dir,
        &profile_record,
        &resolved.effective_capabilities,
    );
    let read_inputs = read_content_inputs(candidates, &mut diagnostics);
    if has_error_diagnostics(&diagnostics) {
        result.diagnostics = diagnostics;
        return Ok(result);
    }

    let adapter_version = adapter_version(&runtime_config.id);
    let fingerprint = build_fingerprint(
        &profile_record,
        &resolved.effective_capabilities,
        &resolved.instruction_fragments,
        &runtime_config,
        &adapter_version,
        &read_inputs,
    );
    let build_id = fingerprint
        .split_once(':')
        .map_or_else(|| fingerprint.clone(), |(_, digest)| digest.to_string());
    let paths = BuildPlanPaths {
        build_dir: runtime_build_dir(&runtime_config.id, &profile_record.id, &build_id),
        home_dir: runtime_home_dir(&runtime_config.id, &profile_record.id, &build_id),
        runtime_pointer: runtime_pointer_dir(&profile_record.id, &runtime_config.id),
    };

    result.plan = Some(ProfileBuildPlan {
        schema_version: BUILD_PLAN_SCHEMA_VERSION,
        profile: profile_record.id.clone(),
        runtime: runtime_config.id.clone(),
        adapter_version,
        fingerprint,
        build_id,
        paths,
        profile_record,
        effective_capabilities: resolved.effective_capabilities,
        instruction_fragments: resolved.instruction_fragments,
        runtime_config,
        content_inputs: read_inputs
            .into_iter()
            .map(|input| input.content_input)
            .collect(),
    });
    result.diagnostics = diagnostics;
    Ok(result)
}

fn select_runtime_config(
    requested_runtime: Option<&str>,
    selected_runtime: Option<&str>,
    runtime_configs: &[ResolvedRuntimeConfig],
    profile_record: &ProfileIndexRecord,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ResolvedRuntimeConfig> {
    let runtime_id = requested_runtime.or(selected_runtime)?;
    let runtime_config = runtime_configs
        .iter()
        .find(|config| config.id == runtime_id)
        .cloned();
    if runtime_config.is_none() {
        diagnostics.push(unavailable_runtime_diagnostic(runtime_id, profile_record));
    }
    runtime_config
}

fn content_input_candidates(
    repo_root: &Path,
    user_state_dir: &Path,
    profile_record: &ProfileIndexRecord,
    effective_capabilities: &[CapabilityIndexRecord],
) -> Vec<ContentInputCandidate> {
    let mut candidates = Vec::new();
    candidates.push(ContentInputCandidate::required(
        0,
        "profile-manifest",
        relative_repo_path(&profile_manifest_path(profile_record)),
        repo_root.join(profile_manifest_path(profile_record)),
    ));

    for capability in effective_capabilities {
        candidates.push(ContentInputCandidate::required(
            1,
            "capability-manifest",
            relative_repo_path(&capability_manifest_path(capability)),
            repo_root.join(capability_manifest_path(capability)),
        ));
        for file_path in capability.files.values() {
            let repo_relative = Path::new(&capability.source_path).join(file_path);
            candidates.push(ContentInputCandidate::required(
                2,
                "capability-file",
                relative_repo_path(&repo_relative),
                repo_root.join(repo_relative),
            ));
        }
    }

    let repo_defaults = PathBuf::from(REPO_DEFAULTS_DIR_NAME).join(RUNTIMES_FILE_NAME);
    candidates.push(ContentInputCandidate::optional(
        3,
        "repo-runtime-defaults",
        relative_repo_path(&repo_defaults),
        repo_root.join(repo_defaults),
    ));
    candidates.push(ContentInputCandidate::optional(
        4,
        "user-config",
        format!("user-state/{USER_CONFIG_FILE_NAME}"),
        user_state_dir.join(USER_CONFIG_FILE_NAME),
    ));

    deduplicate_and_sort(candidates)
}

fn read_content_inputs(
    candidates: Vec<ContentInputCandidate>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<ReadContentInput> {
    let mut read_inputs = Vec::new();
    for candidate in candidates {
        match fs::read(&candidate.absolute_path) {
            Ok(bytes) => {
                let mut input_hasher = FingerprintBuilder::new(BUILD_PLAN_SCHEMA_VERSION);
                input_hasher.write_str(&candidate.role);
                input_hasher.write_str(&candidate.stable_path);
                input_hasher.write_bytes(&bytes);
                read_inputs.push(ReadContentInput {
                    content_input: BuildPlanContentInput {
                        role: candidate.role,
                        path: candidate.stable_path,
                        content_fingerprint: input_hasher.finish_prefixed(),
                    },
                    bytes,
                });
            }
            Err(source) if source.kind() == io::ErrorKind::NotFound && !candidate.required => {}
            Err(source) => diagnostics.push(input_read_diagnostic(&candidate, &source)),
        }
    }
    read_inputs
}

fn build_fingerprint(
    profile_record: &ProfileIndexRecord,
    effective_capabilities: &[CapabilityIndexRecord],
    instruction_fragments: &[ResolvedInstructionFragment],
    runtime_config: &ResolvedRuntimeConfig,
    adapter_version: &str,
    read_inputs: &[ReadContentInput],
) -> String {
    let mut hasher = FingerprintBuilder::new(BUILD_PLAN_SCHEMA_VERSION);
    write_json(&mut hasher, "profile-record", profile_record);
    write_json(
        &mut hasher,
        "effective-capabilities",
        effective_capabilities,
    );
    write_json(&mut hasher, "instruction-fragments", instruction_fragments);
    write_json(&mut hasher, "runtime-config", runtime_config);
    hasher.write_str("adapter-version");
    hasher.write_str(adapter_version);
    for input in read_inputs {
        hasher.write_str("content-input");
        hasher.write_str(&input.content_input.role);
        hasher.write_str(&input.content_input.path);
        hasher.write_bytes(&input.bytes);
    }
    hasher.finish_prefixed()
}

fn write_json<T: Serialize + ?Sized>(hasher: &mut FingerprintBuilder, label: &str, value: &T) {
    let encoded = serde_json::to_vec(value).expect("build plan fingerprint material serializes");
    hasher.write_str(label);
    hasher.write_bytes(&encoded);
}

fn deduplicate_and_sort(candidates: Vec<ContentInputCandidate>) -> Vec<ContentInputCandidate> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for candidate in candidates {
        if seen.insert((
            candidate.order,
            candidate.role.clone(),
            candidate.stable_path.clone(),
        )) {
            deduped.push(candidate);
        }
    }
    deduped.sort_by(|left, right| {
        (left.order, &left.stable_path, &left.role).cmp(&(
            right.order,
            &right.stable_path,
            &right.role,
        ))
    });
    deduped
}

fn adapter_version(runtime: &str) -> String {
    format!("agent-matters:{runtime}:adapter:v1")
}

fn profile_manifest_path(profile_record: &ProfileIndexRecord) -> PathBuf {
    Path::new(&profile_record.source_path).join(MANIFEST_FILE_NAME)
}

fn capability_manifest_path(capability: &CapabilityIndexRecord) -> PathBuf {
    Path::new(&capability.source_path).join(MANIFEST_FILE_NAME)
}

fn relative_repo_path(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn has_error_diagnostics(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
}

fn remove_default_runtime_diagnostics(diagnostics: &mut Vec<Diagnostic>) {
    diagnostics.retain(|diagnostic| {
        !matches!(
            diagnostic.code.as_str(),
            "profile.runtime.ambiguous-default" | "profile.runtime.default-unavailable"
        )
    });
}

fn unavailable_runtime_diagnostic(
    runtime_id: &str,
    profile_record: &ProfileIndexRecord,
) -> Diagnostic {
    let available = profile_record
        .runtimes
        .keys()
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "profile.build-plan.runtime-unavailable",
        format!(
            "runtime `{runtime_id}` is not enabled for profile `{}`",
            profile_record.id
        ),
    )
    .with_location(DiagnosticLocation::manifest_field(
        profile_manifest_path(profile_record),
        "runtimes",
    ))
    .with_recovery_hint(format!("choose one of: {available}"))
}

fn input_read_diagnostic(candidate: &ContentInputCandidate, source: &io::Error) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "profile.build-plan.input-read-failed",
        format!(
            "failed to read build plan input `{}`: {source}",
            candidate.stable_path
        ),
    )
    .with_location(DiagnosticLocation::manifest_path(
        candidate.stable_path.clone(),
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ContentInputCandidate {
    order: u8,
    role: String,
    stable_path: String,
    absolute_path: PathBuf,
    required: bool,
}

impl ContentInputCandidate {
    fn required(
        order: u8,
        role: impl Into<String>,
        stable_path: impl Into<String>,
        absolute_path: PathBuf,
    ) -> Self {
        Self {
            order,
            role: role.into(),
            stable_path: stable_path.into(),
            absolute_path,
            required: true,
        }
    }

    fn optional(
        order: u8,
        role: impl Into<String>,
        stable_path: impl Into<String>,
        absolute_path: PathBuf,
    ) -> Self {
        Self {
            order,
            role: role.into(),
            stable_path: stable_path.into(),
            absolute_path,
            required: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReadContentInput {
    content_input: BuildPlanContentInput,
    bytes: Vec<u8>,
}
