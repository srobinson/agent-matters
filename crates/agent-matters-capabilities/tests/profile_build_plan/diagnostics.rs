use std::fs;
use std::path::Path;

use agent_matters_capabilities::profiles::plan_profile_build;
use agent_matters_core::domain::DiagnosticSeverity;
use tempfile::TempDir;

use crate::common::{build_plan_request, valid_repo};

#[test]
fn missing_referenced_content_file_reports_input_diagnostic() {
    let repo = valid_repo();
    let state = TempDir::new().unwrap();
    fs::remove_file(
        repo.path()
            .join("catalog/instructions/helioy-core/AGENTS.md"),
    )
    .unwrap();

    let result = plan_profile_build(build_plan_request(repo.path(), state.path())).unwrap();

    assert!(result.plan.is_none());
    assert_eq!(result.diagnostics.len(), 1);
    let diagnostic = &result.diagnostics[0];
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(diagnostic.code, "profile.build-plan.input-read-failed");
    assert!(
        diagnostic
            .message
            .contains("catalog/instructions/helioy-core/AGENTS.md")
    );
    assert_eq!(
        diagnostic
            .location
            .as_ref()
            .and_then(|location| location.manifest_path.as_deref()),
        Some(Path::new("catalog/instructions/helioy-core/AGENTS.md"))
    );
}
