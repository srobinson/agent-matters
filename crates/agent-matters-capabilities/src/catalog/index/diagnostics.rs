use std::path::Path;

use agent_matters_core::domain::{Diagnostic, DiagnosticLocation, DiagnosticSeverity};

pub(super) fn corrupt_index_diagnostic(path: &Path, detail: &str) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "catalog.index-corrupt",
        format!(
            "generated catalog index `{}` is corrupt: {detail}",
            path.display()
        ),
    )
    .with_location(DiagnosticLocation::manifest_path(path))
    .with_recovery_hint("delete the generated index or rerun the command to rebuild it")
}
