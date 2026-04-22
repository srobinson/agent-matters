use agent_matters_core::domain::{Diagnostic, DiagnosticSeverity};

pub(super) fn has_error_diagnostics(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
}

pub(super) fn remove_default_runtime_diagnostics(diagnostics: &mut Vec<Diagnostic>) {
    diagnostics.retain(|diagnostic| {
        !matches!(
            diagnostic.code.as_str(),
            "profile.runtime.ambiguous-default" | "profile.runtime.default-unavailable"
        )
    });
}
