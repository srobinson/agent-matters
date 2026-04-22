use std::collections::BTreeSet;
use std::path::Path;

use agent_matters_core::domain::{Diagnostic, DiagnosticSeverity};

use crate::catalog::CatalogDiscovery;
use crate::profiles::{
    CLAUDE_RUNTIME_ID, CODEX_RUNTIME_ID, ResolvedRuntimeConfig, RuntimeAdapter, adapter_for_runtime,
};

use super::{DoctorGeneratedStateSummary, DoctorRequest, DoctorRuntimeAdapterSummary};

mod generated_state;

use generated_state::inspect_generated_state;

pub(super) struct RuntimeInspection {
    pub(super) runtimes: Vec<DoctorRuntimeAdapterSummary>,
    pub(super) generated_state: DoctorGeneratedStateSummary,
}

pub(super) fn inspect_runtime_environment(
    request: &DoctorRequest,
    discovery: &CatalogDiscovery,
    diagnostics: &mut Vec<Diagnostic>,
) -> RuntimeInspection {
    let needed_runtimes = enabled_profile_runtime_ids(discovery);
    let runtimes = [CODEX_RUNTIME_ID, CLAUDE_RUNTIME_ID]
        .into_iter()
        .map(|runtime| inspect_runtime_adapter(runtime, request, &needed_runtimes, diagnostics))
        .collect();
    let generated_state = inspect_generated_state(&request.user_state_dir, diagnostics);

    RuntimeInspection {
        runtimes,
        generated_state,
    }
}

fn inspect_runtime_adapter(
    runtime: &str,
    request: &DoctorRequest,
    needed_runtimes: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> DoctorRuntimeAdapterSummary {
    let Some(adapter) = adapter_for_runtime(runtime) else {
        diagnostics.push(missing_runtime_adapter(runtime));
        return DoctorRuntimeAdapterSummary {
            id: runtime.to_string(),
            adapter_available: false,
            default_config_valid: false,
        };
    };

    let default_config_valid = validate_default_config(adapter, diagnostics);
    if needed_runtimes.contains(runtime)
        && let Some(native_home_dir) = request.native_home_dir.as_deref()
    {
        inspect_credential_sources(adapter, native_home_dir, diagnostics);
    }

    DoctorRuntimeAdapterSummary {
        id: runtime.to_string(),
        adapter_available: true,
        default_config_valid,
    }
}

fn enabled_profile_runtime_ids(discovery: &CatalogDiscovery) -> BTreeSet<String> {
    discovery
        .profiles
        .iter()
        .filter_map(|profile| profile.manifest.runtimes.as_ref())
        .flat_map(|runtimes| runtimes.entries.iter())
        .filter(|(_, runtime)| runtime.enabled)
        .map(|(id, _)| id.to_string())
        .collect()
}

fn validate_default_config(
    adapter: &'static dyn RuntimeAdapter,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let settings = adapter.default_settings();
    let config = ResolvedRuntimeConfig {
        id: adapter.id().to_string(),
        model: settings.model,
    };
    let adapter_diagnostics = adapter.validate_config(&config);
    let valid = !adapter_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error);
    diagnostics.extend(adapter_diagnostics);
    valid
}

fn inspect_credential_sources(
    adapter: &'static dyn RuntimeAdapter,
    native_home_dir: &Path,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(source_dir) = adapter.credential_source_dir(native_home_dir) else {
        return;
    };

    for entry in adapter.credential_symlink_allowlist() {
        let source_path = source_dir.join(&entry.source_name);
        if !source_path.is_file() {
            diagnostics.push(credential_source_missing(adapter.id(), &source_path));
        }
    }
}

fn missing_runtime_adapter(runtime: &str) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "runtime.adapter-missing",
        format!("required runtime adapter `{runtime}` is not registered"),
    )
    .with_recovery_hint("reinstall agent-matters or restore runtime adapter registration")
}

fn credential_source_missing(runtime: &str, source_path: &Path) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Warning,
        "runtime.credential-source-missing",
        format!(
            "`{runtime}` credential source `{}` does not exist",
            source_path.display()
        ),
    )
    .with_recovery_hint(format!(
        "authenticate with `{runtime}` so `{}` exists before using affected profiles",
        source_path.display()
    ))
}
