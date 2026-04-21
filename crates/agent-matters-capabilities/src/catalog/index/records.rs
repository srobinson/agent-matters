use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use agent_matters_core::catalog::{
    CapabilityIndexRecord, CapabilitySourceSummary, ProfileIndexRecord, ProvenanceSummary,
    RequirementSummary, RuntimeCompatibilitySummary,
};
use agent_matters_core::domain::Provenance;

use super::super::{
    CapabilityDiscoverySource, CatalogDiscovery, DiscoveredCapabilityManifest,
    DiscoveredProfileManifest,
};
use super::paths::relative_path;

pub(super) fn capability_record(
    repo_root: &Path,
    entry: &DiscoveredCapabilityManifest,
) -> CapabilityIndexRecord {
    let manifest = &entry.manifest;
    let requirements =
        manifest
            .requires
            .as_ref()
            .map_or_else(RequirementSummary::default, |requires| RequirementSummary {
                capabilities: requires
                    .capabilities
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                env: requires
                    .env
                    .iter()
                    .map(|env| env.name().to_string())
                    .collect(),
            });

    CapabilityIndexRecord {
        id: manifest.id.to_string(),
        kind: manifest.kind.as_str().to_string(),
        summary: manifest.summary.clone(),
        files: manifest.files.entries.clone(),
        source_path: relative_path(repo_root, &entry.directory_path),
        source: capability_source_summary(repo_root, entry),
        runtimes: manifest
            .runtimes
            .entries
            .iter()
            .map(|(runtime, compatibility)| {
                (
                    runtime.to_string(),
                    RuntimeCompatibilitySummary {
                        supported: compatibility.supported,
                        model: None,
                    },
                )
            })
            .collect(),
        provenance: provenance_summary(manifest.origin.as_ref()),
        requirements,
    }
}

fn capability_source_summary(
    repo_root: &Path,
    entry: &DiscoveredCapabilityManifest,
) -> CapabilitySourceSummary {
    match &entry.source {
        CapabilityDiscoverySource::Local => CapabilitySourceSummary {
            kind: "local".to_string(),
            normalized_path: Some(relative_path(repo_root, &entry.directory_path)),
            overlay_path: None,
            vendor_path: None,
        },
        CapabilityDiscoverySource::Imported { vendor_path } => CapabilitySourceSummary {
            kind: "imported".to_string(),
            normalized_path: Some(relative_path(repo_root, &entry.directory_path)),
            overlay_path: None,
            vendor_path: vendor_path
                .as_ref()
                .map(|path| relative_path(repo_root, path)),
        },
        CapabilityDiscoverySource::Overlay {
            target_directory_path,
            vendor_path,
            ..
        } => CapabilitySourceSummary {
            kind: "overlaid".to_string(),
            normalized_path: Some(relative_path(repo_root, target_directory_path)),
            overlay_path: Some(relative_path(repo_root, &entry.directory_path)),
            vendor_path: vendor_path
                .as_ref()
                .map(|path| relative_path(repo_root, path)),
        },
    }
}

pub(super) fn profile_record(
    repo_root: &Path,
    entry: &DiscoveredProfileManifest,
) -> ProfileIndexRecord {
    let manifest = &entry.manifest;
    let runtimes = manifest
        .runtimes
        .as_ref()
        .map_or_else(BTreeMap::new, |runtimes| {
            runtimes
                .entries
                .iter()
                .map(|(runtime, compatibility)| {
                    (
                        runtime.to_string(),
                        RuntimeCompatibilitySummary {
                            supported: compatibility.enabled,
                            model: compatibility.model.clone(),
                        },
                    )
                })
                .collect()
        });
    let default_runtime = manifest
        .runtimes
        .as_ref()
        .and_then(|runtimes| runtimes.default.as_ref())
        .map(ToString::to_string);
    let instruction_markers = manifest
        .instructions_output
        .as_ref()
        .and_then(|output| output.markers);

    ProfileIndexRecord {
        id: manifest.id.to_string(),
        kind: manifest.kind.as_str().to_string(),
        summary: manifest.summary.clone(),
        capabilities: manifest
            .capabilities
            .iter()
            .map(ToString::to_string)
            .collect(),
        instructions: manifest
            .instructions
            .iter()
            .map(ToString::to_string)
            .collect(),
        scope: manifest.scope.clone().unwrap_or_default(),
        source_path: relative_path(repo_root, &entry.directory_path),
        runtimes,
        default_runtime,
        instruction_markers,
        capability_count: manifest.capabilities.len(),
        instruction_count: manifest.instructions.len(),
    }
}

fn provenance_summary(origin: Option<&Provenance>) -> ProvenanceSummary {
    match origin.unwrap_or(&Provenance::Local) {
        Provenance::Local => ProvenanceSummary {
            kind: "local".to_string(),
            source: None,
            locator: None,
            version: None,
        },
        Provenance::External {
            source,
            locator,
            version,
        } => ProvenanceSummary {
            kind: "external".to_string(),
            source: Some(source.clone()),
            locator: Some(locator.clone()),
            version: version.clone(),
        },
        Provenance::Derived {
            source,
            locator,
            version,
        } => ProvenanceSummary {
            kind: "derived".to_string(),
            source: Some(source.clone()),
            locator: Some(locator.clone()),
            version: version.clone(),
        },
        Provenance::Generated {
            source,
            locator,
            version,
        } => ProvenanceSummary {
            kind: "generated".to_string(),
            source: Some(source.clone()),
            locator: locator.clone(),
            version: version.clone(),
        },
    }
}

pub(super) fn duplicate_capability_ids(discovery: &CatalogDiscovery) -> BTreeSet<String> {
    duplicate_ids(
        discovery
            .capabilities
            .iter()
            .map(|entry| entry.manifest.id.to_string()),
    )
}

pub(super) fn duplicate_profile_ids(discovery: &CatalogDiscovery) -> BTreeSet<String> {
    duplicate_ids(
        discovery
            .profiles
            .iter()
            .map(|entry| entry.manifest.id.to_string()),
    )
}

fn duplicate_ids(ids: impl Iterator<Item = String>) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    let mut duplicates = BTreeSet::new();
    for id in ids {
        if !seen.insert(id.clone()) {
            duplicates.insert(id);
        }
    }
    duplicates
}
