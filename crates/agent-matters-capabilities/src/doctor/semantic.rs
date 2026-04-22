use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::Path;

use agent_matters_core::catalog::path_is_in_repo_vendor;
use agent_matters_core::domain::{Diagnostic, DiagnosticLocation, DiagnosticSeverity};

use crate::catalog::{
    CapabilityDiscoverySource, CatalogDiscovery, DiscoveredCapabilityManifest,
    DiscoveredProfileManifest,
};

pub(super) fn validate_catalog_semantics(
    repo_root: &Path,
    discovery: &CatalogDiscovery,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let capability_entries = unique_capability_entries(discovery);
    let env_names = observed_env_names();

    validate_capability_files(discovery, diagnostics);
    validate_capability_requirements(discovery, &capability_entries, diagnostics);
    validate_profile_references(discovery, &capability_entries, diagnostics);
    validate_profile_requirement_satisfaction(discovery, &capability_entries, diagnostics);
    validate_env_requirements(discovery, &env_names, diagnostics);
    validate_import_sources(repo_root, discovery, diagnostics);
}

fn unique_capability_entries(
    discovery: &CatalogDiscovery,
) -> BTreeMap<String, &DiscoveredCapabilityManifest> {
    let mut counts = BTreeMap::<String, usize>::new();
    for entry in &discovery.capabilities {
        *counts.entry(entry.manifest.id.to_string()).or_default() += 1;
    }
    discovery
        .capabilities
        .iter()
        .filter_map(|entry| {
            let id = entry.manifest.id.to_string();
            (counts.get(&id) == Some(&1)).then_some((id, entry))
        })
        .collect()
}

fn observed_env_names() -> BTreeSet<String> {
    std::env::vars_os()
        .filter_map(|(name, _)| name.into_string().ok())
        .collect()
}

fn validate_capability_files(discovery: &CatalogDiscovery, diagnostics: &mut Vec<Diagnostic>) {
    for entry in &discovery.capabilities {
        for (key, value) in &entry.manifest.files.entries {
            let path = entry.directory_path.join(value);
            if !path.is_file() {
                diagnostics.push(missing_capability_file(entry, key, value, &path));
            }
        }
    }
}

fn validate_capability_requirements(
    discovery: &CatalogDiscovery,
    capability_entries: &BTreeMap<String, &DiscoveredCapabilityManifest>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for entry in &discovery.capabilities {
        let Some(requires) = &entry.manifest.requires else {
            continue;
        };
        for required in &requires.capabilities {
            let id = required.to_string();
            if !capability_entries.contains_key(&id) {
                diagnostics.push(missing_required_capability(entry, &id));
            }
        }
    }
}

fn validate_profile_references(
    discovery: &CatalogDiscovery,
    capability_entries: &BTreeMap<String, &DiscoveredCapabilityManifest>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for entry in &discovery.profiles {
        for id in &entry.manifest.capabilities {
            let id = id.to_string();
            if !capability_entries.contains_key(&id) {
                diagnostics.push(missing_profile_capability(entry, "capabilities", &id));
            }
        }
        for id in &entry.manifest.instructions {
            let id = id.to_string();
            if !capability_entries.contains_key(&id) {
                diagnostics.push(missing_profile_capability(entry, "instructions", &id));
            }
        }
    }
}

fn validate_profile_requirement_satisfaction(
    discovery: &CatalogDiscovery,
    capability_entries: &BTreeMap<String, &DiscoveredCapabilityManifest>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for profile in &discovery.profiles {
        let included = profile_capability_inventory(profile);
        for id in &included {
            let Some(entry) = capability_entries.get(id) else {
                continue;
            };
            let Some(requires) = &entry.manifest.requires else {
                continue;
            };
            for required in &requires.capabilities {
                let required_id = required.to_string();
                if !included.contains(&required_id) && capability_entries.contains_key(&required_id)
                {
                    diagnostics.push(profile_missing_required_capability(
                        profile,
                        entry,
                        &required_id,
                    ));
                }
            }
        }
    }
}

fn profile_capability_inventory(entry: &DiscoveredProfileManifest) -> BTreeSet<String> {
    entry
        .manifest
        .capabilities
        .iter()
        .chain(entry.manifest.instructions.iter())
        .map(ToString::to_string)
        .collect()
}

fn validate_env_requirements(
    discovery: &CatalogDiscovery,
    env_names: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for entry in &discovery.capabilities {
        let Some(requires) = &entry.manifest.requires else {
            continue;
        };
        for required in &requires.env {
            if !env_names.contains(required.name()) {
                diagnostics.push(missing_required_env(entry, required.name()));
            }
        }
    }
}

fn validate_import_sources(
    repo_root: &Path,
    discovery: &CatalogDiscovery,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for entry in &discovery.capabilities {
        if source_requires_import_provenance(entry)
            && !entry
                .manifest
                .origin
                .as_ref()
                .is_some_and(|origin| origin.requires_vendor_record())
        {
            diagnostics.push(missing_import_provenance(entry));
        }

        let Some(vendor_path) = capability_vendor_path(entry) else {
            if source_requires_import_provenance(entry) {
                diagnostics.push(missing_vendor_record(entry, None));
            }
            continue;
        };
        if !path_is_in_repo_vendor(repo_root, vendor_path) {
            diagnostics.push(invalid_vendor_record_path(entry, vendor_path));
            continue;
        }
        validate_vendor_record(entry, vendor_path, diagnostics);
    }
}

fn source_requires_import_provenance(entry: &DiscoveredCapabilityManifest) -> bool {
    match &entry.source {
        CapabilityDiscoverySource::Local => false,
        CapabilityDiscoverySource::Imported { .. } => true,
        CapabilityDiscoverySource::Overlay { target_origin, .. } => target_origin
            .as_ref()
            .is_some_and(|origin| origin.requires_vendor_record()),
    }
}

fn capability_vendor_path(entry: &DiscoveredCapabilityManifest) -> Option<&Path> {
    match &entry.source {
        CapabilityDiscoverySource::Imported { vendor_path }
        | CapabilityDiscoverySource::Overlay { vendor_path, .. } => vendor_path.as_deref(),
        CapabilityDiscoverySource::Local => None,
    }
}

fn validate_vendor_record(
    entry: &DiscoveredCapabilityManifest,
    vendor_path: &Path,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Err(source) = fs::metadata(vendor_path) {
        if source.kind() == io::ErrorKind::NotFound {
            diagnostics.push(missing_vendor_record(entry, Some(vendor_path)));
        } else {
            diagnostics.push(vendor_record_read_failed(entry, vendor_path, &source));
        }
        return;
    }

    let mut has_readable_file = false;
    scan_vendor_records(entry, vendor_path, diagnostics, &mut has_readable_file);

    if !has_readable_file {
        diagnostics.push(missing_vendor_record(entry, Some(vendor_path)));
    }
}

fn scan_vendor_records(
    entry: &DiscoveredCapabilityManifest,
    directory: &Path,
    diagnostics: &mut Vec<Diagnostic>,
    has_readable_file: &mut bool,
) {
    let records = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            return;
        }
        Err(source) => {
            diagnostics.push(vendor_record_read_failed(entry, directory, &source));
            return;
        }
    };

    for record in records {
        let record = match record {
            Ok(record) => record,
            Err(source) => {
                diagnostics.push(vendor_record_read_failed(entry, directory, &source));
                continue;
            }
        };
        let path = record.path();
        let file_type = match record.file_type() {
            Ok(file_type) => file_type,
            Err(source) => {
                diagnostics.push(vendor_record_read_failed(entry, &path, &source));
                continue;
            }
        };
        if file_type.is_dir() {
            scan_vendor_records(entry, &path, diagnostics, has_readable_file);
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        match fs::read(&path) {
            Ok(_) => *has_readable_file = true,
            Err(source) => diagnostics.push(vendor_record_read_failed(entry, &path, &source)),
        }
    }
}

fn missing_capability_file(
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

fn missing_required_capability(entry: &DiscoveredCapabilityManifest, id: &str) -> Diagnostic {
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

fn missing_required_env(entry: &DiscoveredCapabilityManifest, name: &str) -> Diagnostic {
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

fn profile_missing_required_capability(
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

fn missing_profile_capability(
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

fn missing_import_provenance(entry: &DiscoveredCapabilityManifest) -> Diagnostic {
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

fn missing_vendor_record(
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

fn invalid_vendor_record_path(entry: &DiscoveredCapabilityManifest, path: &Path) -> Diagnostic {
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

fn vendor_record_read_failed(
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
