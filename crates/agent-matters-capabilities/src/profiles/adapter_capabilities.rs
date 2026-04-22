use std::fmt;
use std::path::{Path, PathBuf};

use agent_matters_core::catalog::{CapabilityIndexRecord, MANIFEST_FILE_NAME};
use agent_matters_core::domain::{Diagnostic, DiagnosticLocation, DiagnosticSeverity};

#[derive(Debug, Clone, Copy)]
pub(crate) struct RuntimeCapabilitySupport {
    display_name: &'static str,
    diagnostic_prefix: &'static str,
    mcp_config_label: &'static str,
    config_render_label: &'static str,
}

impl RuntimeCapabilitySupport {
    pub(crate) const fn new(
        display_name: &'static str,
        diagnostic_prefix: &'static str,
        mcp_config_label: &'static str,
        config_render_label: &'static str,
    ) -> Self {
        Self {
            display_name,
            diagnostic_prefix,
            mcp_config_label,
            config_render_label,
        }
    }

    pub(crate) fn expected_file_mapping<'a>(
        &self,
        capability: &'a CapabilityIndexRecord,
        expected_role: &str,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<&'a str> {
        for role in capability.files.keys() {
            if role != expected_role {
                diagnostics.push(self.unsupported_file_mapping(capability, role, expected_role));
            }
        }

        capability
            .files
            .get(expected_role)
            .map(String::as_str)
            .or_else(|| {
                diagnostics.push(self.missing_file_mapping(capability, expected_role));
                None
            })
    }

    pub(crate) fn unsupported_capability_kind(
        &self,
        capability: &CapabilityIndexRecord,
    ) -> Diagnostic {
        Diagnostic::new(
            DiagnosticSeverity::Error,
            format!("{}.capability-kind-unsupported", self.diagnostic_prefix),
            format!(
                "{} adapter does not support capability kind `{}` for `{}`",
                self.display_name, capability.kind, capability.id
            ),
        )
        .with_location(DiagnosticLocation::manifest_path(capability_manifest_path(
            capability,
        )))
    }

    pub(crate) fn file_read_failed(
        &self,
        capability: &CapabilityIndexRecord,
        source_file: &str,
        source: &std::io::Error,
    ) -> Diagnostic {
        Diagnostic::new(
            DiagnosticSeverity::Error,
            format!("{}.file-read-failed", self.diagnostic_prefix),
            format!(
                "failed to read `{}` file `{}` for {} adapter: {source}",
                capability.id, source_file, self.display_name
            ),
        )
        .with_location(DiagnosticLocation::manifest_path(
            Path::new(&capability.source_path).join(source_file),
        ))
    }

    pub(crate) fn mcp_manifest_invalid(
        &self,
        capability: &CapabilityIndexRecord,
        source_file: &str,
        source: &toml::de::Error,
    ) -> Diagnostic {
        Diagnostic::new(
            DiagnosticSeverity::Error,
            format!("{}.mcp-config-invalid", self.diagnostic_prefix),
            format!(
                "failed to parse `{}` MCP file `{}` for {}: {source}",
                capability.id, source_file, self.mcp_config_label
            ),
        )
        .with_location(DiagnosticLocation::manifest_path(
            Path::new(&capability.source_path).join(source_file),
        ))
    }

    pub(crate) fn config_render_failed(&self, source: &dyn fmt::Display) -> Diagnostic {
        Diagnostic::new(
            DiagnosticSeverity::Error,
            format!("{}.config-render-failed", self.diagnostic_prefix),
            format!("failed to render {}: {source}", self.config_render_label),
        )
    }

    fn unsupported_file_mapping(
        &self,
        capability: &CapabilityIndexRecord,
        actual_role: &str,
        expected_role: &str,
    ) -> Diagnostic {
        Diagnostic::new(
            DiagnosticSeverity::Error,
            format!("{}.file-mapping-unsupported", self.diagnostic_prefix),
            format!(
                "{} adapter does not support `{}` file mapping `{}`; expected `{}`",
                self.display_name, capability.id, actual_role, expected_role
            ),
        )
        .with_location(DiagnosticLocation::manifest_field(
            capability_manifest_path(capability),
            format!("files.{actual_role}"),
        ))
        .with_recovery_hint("use the supported file role for this capability kind")
    }

    fn missing_file_mapping(
        &self,
        capability: &CapabilityIndexRecord,
        expected_role: &str,
    ) -> Diagnostic {
        Diagnostic::new(
            DiagnosticSeverity::Error,
            format!("{}.file-mapping-missing", self.diagnostic_prefix),
            format!(
                "{} adapter needs `{}` file mapping `{}`",
                self.display_name, capability.id, expected_role
            ),
        )
        .with_location(DiagnosticLocation::manifest_path(capability_manifest_path(
            capability,
        )))
    }
}

pub(crate) fn capability_target_path(
    capability: &CapabilityIndexRecord,
    target_dir: &str,
    source_file: &str,
) -> PathBuf {
    PathBuf::from(target_dir)
        .join(capability_body(&capability.id))
        .join(source_file)
}

pub(crate) fn capability_body(id: &str) -> &str {
    id.split_once(':').map_or(id, |(_, body)| body)
}

pub(crate) fn path_string(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn capability_manifest_path(capability: &CapabilityIndexRecord) -> PathBuf {
    Path::new(&capability.source_path).join(MANIFEST_FILE_NAME)
}
