use std::collections::BTreeSet;

use agent_matters_core::domain::Diagnostic;

use crate::catalog::{CatalogDiscovery, DiscoveredProfileManifest};

use super::CapabilityEntries;
use super::diagnostics::{missing_profile_capability, profile_missing_required_capability};

pub(super) fn validate_profile_references(
    discovery: &CatalogDiscovery,
    capability_entries: &CapabilityEntries<'_>,
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

pub(super) fn validate_profile_requirement_satisfaction(
    discovery: &CatalogDiscovery,
    capability_entries: &CapabilityEntries<'_>,
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
