//! Validate requirements declared by resolved profile capabilities.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use agent_matters_core::catalog::{CapabilityIndexRecord, MANIFEST_FILE_NAME};
use agent_matters_core::domain::{
    Diagnostic, DiagnosticLocation, DiagnosticSeverity, EnvVarPresence,
};
use serde::Serialize;

use super::ResolveProfileResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProfileRequirementValidationMode {
    Compile,
    Use,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProfileRequirementValidationResult {
    pub capability_checks: Vec<CapabilityRequirementCheck>,
    pub env_checks: Vec<EnvRequirementCheck>,
    pub diagnostics: Vec<Diagnostic>,
}

impl ProfileRequirementValidationResult {
    pub fn has_error_diagnostics(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityRequirementCheck {
    pub capability: String,
    pub required_by: String,
    pub status: RequirementPresence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EnvRequirementCheck {
    pub name: String,
    pub required_by: String,
    pub status: EnvVarPresence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RequirementPresence {
    Present,
    Missing,
}

pub fn validate_profile_requirements(
    resolved: &ResolveProfileResult,
    env: &BTreeMap<String, String>,
    mode: ProfileRequirementValidationMode,
) -> ProfileRequirementValidationResult {
    let included_capabilities = resolved
        .effective_capabilities
        .iter()
        .map(|record| record.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut result = ProfileRequirementValidationResult {
        capability_checks: Vec::new(),
        env_checks: Vec::new(),
        diagnostics: Vec::new(),
    };

    for record in &resolved.effective_capabilities {
        validate_capability_dependencies(record, &included_capabilities, &mut result);
        validate_env_requirements(record, env, mode, &mut result);
    }

    result
}

fn validate_capability_dependencies(
    record: &CapabilityIndexRecord,
    included_capabilities: &BTreeSet<&str>,
    result: &mut ProfileRequirementValidationResult,
) {
    for required in &record.requirements.capabilities {
        let status = if included_capabilities.contains(required.as_str()) {
            RequirementPresence::Present
        } else {
            RequirementPresence::Missing
        };
        result.capability_checks.push(CapabilityRequirementCheck {
            capability: required.clone(),
            required_by: record.id.clone(),
            status,
        });

        if status == RequirementPresence::Missing {
            result
                .diagnostics
                .push(missing_required_capability(record, required));
        }
    }
}

fn validate_env_requirements(
    record: &CapabilityIndexRecord,
    env: &BTreeMap<String, String>,
    mode: ProfileRequirementValidationMode,
    result: &mut ProfileRequirementValidationResult,
) {
    for required in &record.requirements.env {
        let status = if env.contains_key(required) {
            EnvVarPresence::Present
        } else {
            EnvVarPresence::Missing
        };
        result.env_checks.push(EnvRequirementCheck {
            name: required.clone(),
            required_by: record.id.clone(),
            status,
        });

        if status == EnvVarPresence::Missing {
            result
                .diagnostics
                .push(missing_required_env(record, required, mode));
        }
    }
}

fn missing_required_capability(record: &CapabilityIndexRecord, required: &str) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "profile.required-capability-missing",
        format!(
            "capability `{}` requires missing capability `{required}`",
            record.id
        ),
    )
    .with_location(DiagnosticLocation::manifest_field(
        capability_manifest_path(record),
        "requires.capabilities",
    ))
    .with_recovery_hint("add the required capability explicitly to the profile manifest")
}

fn missing_required_env(
    record: &CapabilityIndexRecord,
    required: &str,
    mode: ProfileRequirementValidationMode,
) -> Diagnostic {
    let severity = match mode {
        ProfileRequirementValidationMode::Compile => DiagnosticSeverity::Warning,
        ProfileRequirementValidationMode::Use => DiagnosticSeverity::Error,
    };

    Diagnostic::new(
        severity,
        "profile.required-env-missing",
        format!(
            "capability `{}` requires missing environment variable `{required}`",
            record.id
        ),
    )
    .with_location(DiagnosticLocation::manifest_field(
        capability_manifest_path(record),
        "requires.env",
    ))
    .with_recovery_hint("set the environment variable before using this profile")
}

fn capability_manifest_path(record: &CapabilityIndexRecord) -> PathBuf {
    PathBuf::from(&record.source_path).join(MANIFEST_FILE_NAME)
}
