use agent_matters_core::catalog::ProfileIndexRecord;
use agent_matters_core::domain::{Diagnostic, DiagnosticLocation, DiagnosticSeverity};

use super::super::ResolvedRuntimeConfig;
use super::inputs::profile_manifest_path;

pub(super) fn select_runtime_config(
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
