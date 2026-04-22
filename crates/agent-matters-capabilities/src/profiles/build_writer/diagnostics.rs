use std::io;
use std::path::Path;

use agent_matters_core::domain::{Diagnostic, DiagnosticLocation, DiagnosticSeverity};

pub(super) fn write_diagnostic(action: &str, path: &Path, source: &io::Error) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "profile.build.write-failed",
        format!("failed to {action} `{}`: {source}", path.display()),
    )
    .with_location(DiagnosticLocation::manifest_path(path))
    .with_recovery_hint("check permissions and remove any non-directory path at that location")
}

pub(super) fn has_error_diagnostics(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
}
