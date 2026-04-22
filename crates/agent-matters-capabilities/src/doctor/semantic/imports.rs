use std::fs;
use std::io;
use std::path::Path;

use agent_matters_core::catalog::path_is_in_repo_vendor;
use agent_matters_core::domain::Diagnostic;

use crate::catalog::{CapabilityDiscoverySource, CatalogDiscovery, DiscoveredCapabilityManifest};

use super::diagnostics::{
    invalid_vendor_record_path, missing_import_provenance, missing_vendor_record,
    vendor_record_read_failed,
};

pub(super) fn validate_import_sources(
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
