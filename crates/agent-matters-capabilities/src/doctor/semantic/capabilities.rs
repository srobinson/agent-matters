use agent_matters_core::domain::Diagnostic;

use crate::catalog::CatalogDiscovery;

use super::CapabilityEntries;
use super::diagnostics::{missing_capability_file, missing_required_capability};

pub(super) fn unique_capability_entries(discovery: &CatalogDiscovery) -> CapabilityEntries<'_> {
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
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

pub(super) fn validate_capability_files(
    discovery: &CatalogDiscovery,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for entry in &discovery.capabilities {
        for (key, value) in &entry.manifest.files.entries {
            let path = entry.directory_path.join(value);
            if !path.is_file() {
                diagnostics.push(missing_capability_file(entry, key, value, &path));
            }
        }
    }
}

pub(super) fn validate_capability_requirements(
    discovery: &CatalogDiscovery,
    capability_entries: &CapabilityEntries<'_>,
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
