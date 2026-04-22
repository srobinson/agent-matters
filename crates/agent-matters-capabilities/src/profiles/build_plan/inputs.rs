use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use agent_matters_core::catalog::{CapabilityIndexRecord, MANIFEST_FILE_NAME, ProfileIndexRecord};
use agent_matters_core::config::{
    MARKERS_FILE_NAME, REPO_DEFAULTS_DIR_NAME, RUNTIMES_FILE_NAME, USER_CONFIG_FILE_NAME,
};
use agent_matters_core::domain::{Diagnostic, DiagnosticLocation, DiagnosticSeverity};
use agent_matters_core::runtime::{BUILD_PLAN_SCHEMA_VERSION, FingerprintBuilder};

use super::types::BuildPlanContentInput;

pub(super) fn content_input_candidates(
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
    let repo_markers = PathBuf::from(REPO_DEFAULTS_DIR_NAME).join(MARKERS_FILE_NAME);
    candidates.push(ContentInputCandidate::optional(
        4,
        "repo-marker-defaults",
        relative_repo_path(&repo_markers),
        repo_root.join(repo_markers),
    ));
    candidates.push(ContentInputCandidate::optional(
        5,
        "user-config",
        format!("user-state/{USER_CONFIG_FILE_NAME}"),
        user_state_dir.join(USER_CONFIG_FILE_NAME),
    ));

    deduplicate_and_sort(candidates)
}

pub(super) fn read_content_inputs(
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

pub(super) fn profile_manifest_path(profile_record: &ProfileIndexRecord) -> PathBuf {
    Path::new(&profile_record.source_path).join(MANIFEST_FILE_NAME)
}

fn capability_manifest_path(capability: &CapabilityIndexRecord) -> PathBuf {
    Path::new(&capability.source_path).join(MANIFEST_FILE_NAME)
}

fn relative_repo_path(path: &Path) -> String {
    path.to_string_lossy().to_string()
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
pub(super) struct ContentInputCandidate {
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
pub(super) struct ReadContentInput {
    pub(super) content_input: BuildPlanContentInput,
    pub(super) bytes: Vec<u8>,
}
