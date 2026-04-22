use std::io;
use std::path::Path;

use agent_matters_core::domain::{Diagnostic, DiagnosticLocation, DiagnosticSeverity};

use crate::catalog::{DiscoveredCapabilityManifest, DiscoveredProfileManifest};

pub(super) fn missing_capability_file(
    entry: &DiscoveredCapabilityManifest,
    key: &str,
    value: &str,
    path: &Path,
) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "catalog.capability-file-missing",
        format!(
            "capability `{}` references missing file `{}` at `{}`",
            entry.manifest.id,
            value,
            path.display()
        ),
    )
    .with_location(DiagnosticLocation::manifest_field(
        &entry.manifest_path,
        format!("files.{key}"),
    ))
    .with_recovery_hint("add the referenced file or update the capability manifest")
}

pub(super) fn missing_required_capability(
    entry: &DiscoveredCapabilityManifest,
    id: &str,
) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "catalog.capability-requirement-not-found",
        format!(
            "capability `{}` requires missing capability `{id}`",
            entry.manifest.id
        ),
    )
    .with_location(DiagnosticLocation::manifest_field(
        &entry.manifest_path,
        "requires.capabilities",
    ))
    .with_recovery_hint("add the required capability or remove the requirement")
}

pub(super) fn missing_required_env(entry: &DiscoveredCapabilityManifest, name: &str) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Warning,
        "catalog.required-env-missing",
        format!(
            "capability `{}` requires missing environment variable `{name}`",
            entry.manifest.id
        ),
    )
    .with_location(DiagnosticLocation::manifest_field(
        &entry.manifest_path,
        "requires.env",
    ))
    .with_recovery_hint("set the environment variable before compiling or using affected profiles")
}

pub(super) fn profile_missing_required_capability(
    profile: &DiscoveredProfileManifest,
    entry: &DiscoveredCapabilityManifest,
    required: &str,
) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "profile.required-capability-missing",
        format!(
            "profile `{}` includes capability `{}` without required capability `{required}`",
            profile.manifest.id, entry.manifest.id
        ),
    )
    .with_location(DiagnosticLocation::manifest_field(
        &profile.manifest_path,
        "capabilities",
    ))
    .with_recovery_hint("add the required capability to the profile or remove the requirement")
}

pub(super) fn missing_profile_capability(
    entry: &DiscoveredProfileManifest,
    field: &'static str,
    id: &str,
) -> Diagnostic {
    let code = if field == "instructions" {
        "profile.instruction-not-found"
    } else {
        "profile.capability-not-found"
    };
    Diagnostic::new(
        DiagnosticSeverity::Error,
        code,
        format!(
            "profile `{}` references missing capability `{id}` in `{field}`",
            entry.manifest.id
        ),
    )
    .with_location(DiagnosticLocation::manifest_field(
        &entry.manifest_path,
        field,
    ))
    .with_recovery_hint("run `agent-matters capabilities list` to inspect exact capability ids")
}

pub(super) fn missing_import_provenance(entry: &DiscoveredCapabilityManifest) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "catalog.import-provenance-missing",
        format!(
            "capability `{}` is imported or derived but is missing external provenance",
            entry.manifest.id
        ),
    )
    .with_location(DiagnosticLocation::manifest_field(
        &entry.manifest_path,
        "origin",
    ))
    .with_recovery_hint("restore the imported or derived capability origin metadata")
}

pub(super) fn missing_vendor_record(
    entry: &DiscoveredCapabilityManifest,
    vendor_path: Option<&Path>,
) -> Diagnostic {
    let detail = vendor_path.map_or_else(
        || "without a vendor path".to_string(),
        |path| format!("at `{}`", path.display()),
    );
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "catalog.vendor-record-missing",
        format!(
            "capability `{}` is missing its vendor record {detail}",
            entry.manifest.id
        ),
    )
    .with_location(DiagnosticLocation::manifest_field(
        &entry.manifest_path,
        "origin",
    ))
    .with_recovery_hint("restore the vendor record or reimport the capability")
}

pub(super) fn invalid_vendor_record_path(
    entry: &DiscoveredCapabilityManifest,
    path: &Path,
) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "catalog.vendor-record-path-invalid",
        format!(
            "capability `{}` resolves vendor record path outside repository vendor storage at `{}`",
            entry.manifest.id,
            path.display()
        ),
    )
    .with_location(DiagnosticLocation::manifest_field(
        &entry.manifest_path,
        "origin",
    ))
    .with_recovery_hint("use relative origin source and locator values inside the vendor directory")
}

pub(super) fn vendor_record_read_failed(
    entry: &DiscoveredCapabilityManifest,
    path: &Path,
    source: &io::Error,
) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "catalog.vendor-record-read-failed",
        format!(
            "failed to read vendor record for capability `{}` at `{}`: {source}",
            entry.manifest.id,
            path.display()
        ),
    )
    .with_location(DiagnosticLocation::manifest_field(
        &entry.manifest_path,
        "origin",
    ))
    .with_recovery_hint("fix permissions or restore the vendor record")
}
