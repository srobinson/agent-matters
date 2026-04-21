//! Full copy overlay resolution for discovered capability manifests.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use agent_matters_core::catalog::{
    MANIFEST_FILE_NAME, OVERLAYS_DIR_NAME, capability_kind_dir_name, known_capability_dir_names,
};
use agent_matters_core::domain::{
    CapabilityKind, Diagnostic, DiagnosticLocation, DiagnosticSeverity,
};
use agent_matters_core::manifest::CapabilityManifest;

use super::discovery::{
    CapabilityDiscoverySource, CatalogDiscovery, DiscoveredCapabilityManifest, load_manifest,
    read_dir_if_present, report_capability_kind_mismatches, report_missing_manifest,
    vendor_record_path,
};

struct OverlayCandidate {
    manifest: CapabilityManifest,
    manifest_path: PathBuf,
    directory_path: PathBuf,
}

pub(super) fn discover_overlays(repo_root: &Path, discovery: &mut CatalogDiscovery) {
    report_unknown_overlay_dirs(repo_root, &mut discovery.diagnostics);
    let mut overlays = BTreeMap::<String, Vec<OverlayCandidate>>::new();

    for kind in CapabilityKind::all() {
        let root = repo_root
            .join(OVERLAYS_DIR_NAME)
            .join(capability_kind_dir_name(*kind));

        let Some(entries) = read_dir_if_present(&root, &mut discovery.diagnostics) else {
            continue;
        };

        for path in entries {
            if !path.is_dir() {
                continue;
            }
            let manifest_path = path.join(MANIFEST_FILE_NAME);
            match load_manifest::<CapabilityManifest>(&manifest_path, &mut discovery.diagnostics) {
                Some(manifest) => {
                    report_capability_kind_mismatches(
                        *kind,
                        &manifest,
                        &manifest_path,
                        &mut discovery.diagnostics,
                    );
                    overlays
                        .entry(manifest.id.to_string())
                        .or_default()
                        .push(OverlayCandidate {
                            manifest,
                            manifest_path,
                            directory_path: path,
                        });
                }
                None if !manifest_path.exists() => report_missing_manifest(
                    &manifest_path,
                    "capability overlay directory is missing manifest.toml",
                    &mut discovery.diagnostics,
                ),
                None => {}
            }
        }
    }

    apply_overlays(repo_root, overlays, discovery);
}

fn report_unknown_overlay_dirs(repo_root: &Path, diagnostics: &mut Vec<Diagnostic>) {
    let root = repo_root.join(OVERLAYS_DIR_NAME);
    let Some(entries) = read_dir_if_present(&root, diagnostics) else {
        return;
    };

    for path in entries {
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if known_capability_dir_names().contains(&name) {
            continue;
        }
        diagnostics.push(
            Diagnostic::new(
                DiagnosticSeverity::Warning,
                "catalog.overlay-unknown-folder",
                format!("unknown overlay folder `{}`", path.display()),
            )
            .with_location(DiagnosticLocation::manifest_path(path))
            .with_recovery_hint(format!(
                "use one of: {}",
                known_capability_dir_names().join(", ")
            )),
        );
    }
}

fn apply_overlays(
    repo_root: &Path,
    overlays: BTreeMap<String, Vec<OverlayCandidate>>,
    discovery: &mut CatalogDiscovery,
) {
    for (id, candidates) in overlays {
        let matches: Vec<usize> = discovery
            .capabilities
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| (entry.manifest.id.to_string() == id).then_some(index))
            .collect();

        match matches.as_slice() {
            [] => {
                for candidate in candidates {
                    discovery
                        .diagnostics
                        .push(overlay_target_missing_diagnostic(
                            &id,
                            &candidate.manifest_path,
                        ));
                }
            }
            [_] if candidates.len() > 1 => {
                for candidate in candidates {
                    discovery
                        .diagnostics
                        .push(overlay_duplicate_diagnostic(&id, &candidate.manifest_path));
                }
            }
            [index] => {
                if let Some(candidate) = candidates.into_iter().next() {
                    apply_overlay(repo_root, candidate, *index, discovery);
                }
            }
            _ => {
                for candidate in candidates {
                    discovery
                        .diagnostics
                        .push(overlay_target_ambiguous_diagnostic(
                            &id,
                            &candidate.manifest_path,
                        ));
                }
            }
        }
    }
}

fn apply_overlay(
    repo_root: &Path,
    candidate: OverlayCandidate,
    target_index: usize,
    discovery: &mut CatalogDiscovery,
) {
    let target = discovery.capabilities[target_index].clone();
    discovery.capabilities[target_index] = DiscoveredCapabilityManifest {
        source: CapabilityDiscoverySource::Overlay {
            target_manifest_path: target.manifest_path,
            target_directory_path: target.directory_path,
            target_origin: target.manifest.origin.clone(),
            vendor_path: target
                .manifest
                .origin
                .as_ref()
                .and_then(|origin| vendor_record_path(repo_root, origin)),
        },
        manifest: candidate.manifest,
        manifest_path: candidate.manifest_path,
        directory_path: candidate.directory_path,
    };
}

fn overlay_target_missing_diagnostic(id: &str, overlay_path: &Path) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "catalog.overlay-target-missing",
        format!("overlay for capability `{id}` has no normalized catalog target"),
    )
    .with_location(DiagnosticLocation::manifest_field(overlay_path, "id"))
    .with_recovery_hint("add the target capability under catalog before overlaying it")
}

fn overlay_duplicate_diagnostic(id: &str, overlay_path: &Path) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "catalog.overlay-duplicate",
        format!("multiple overlays match capability `{id}`"),
    )
    .with_location(DiagnosticLocation::manifest_field(overlay_path, "id"))
    .with_recovery_hint("keep only one overlay directory for each capability id")
}

fn overlay_target_ambiguous_diagnostic(id: &str, overlay_path: &Path) -> Diagnostic {
    Diagnostic::new(
        DiagnosticSeverity::Error,
        "catalog.overlay-target-ambiguous",
        format!("overlay for capability `{id}` matches multiple normalized catalog targets"),
    )
    .with_location(DiagnosticLocation::manifest_field(overlay_path, "id"))
    .with_recovery_hint("resolve duplicate catalog capability ids before applying the overlay")
}
