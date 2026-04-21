//! Capability manifest schema.
//!
//! Capabilities are typed catalog entries with file references, runtime
//! compatibility, optional requirements, and optional import provenance.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::domain::{CapabilityId, CapabilityKind, Provenance, Requirements, RuntimeId};

/// Authored `catalog/<kind>/<id>/manifest.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityManifest {
    pub id: CapabilityId,
    pub kind: CapabilityKind,
    pub summary: String,
    pub files: CapabilityFilesManifest,
    pub runtimes: CapabilityRuntimesManifest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires: Option<CapabilityRequirementsManifest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<OriginManifest>,
}

/// `[files]` table. The file keys are capability kind specific, so the MVP
/// schema preserves them as a deterministic map.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityFilesManifest {
    #[serde(flatten)]
    pub entries: BTreeMap<String, String>,
}

/// `[runtimes]` table keyed by runtime id.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityRuntimesManifest {
    #[serde(flatten)]
    pub entries: BTreeMap<RuntimeId, CapabilityRuntimeManifest>,
}

/// `[runtimes.<name>]` table. `supported` has no default because capability
/// compatibility must be explicit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityRuntimeManifest {
    pub supported: bool,
}

/// Optional `[requires]` table. Dependency validation is a later compile time
/// concern; this schema reuses the semantic requirements type.
pub type CapabilityRequirementsManifest = Requirements;

/// Optional `[origin]` table used by imported or derived capabilities.
pub type OriginManifest = Provenance;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_manifest_parses_minimum_document() {
        let src = r#"
            id = "mcp:linear"
            kind = "mcp"
            summary = "Linear MCP server."

            [files]
            manifest = "server.toml"

            [runtimes.codex]
            supported = true
        "#;

        let parsed: CapabilityManifest = toml::from_str(src).unwrap();

        assert_eq!(parsed.id.to_string(), "mcp:linear");
        assert_eq!(parsed.kind, CapabilityKind::Mcp);
        assert_eq!(
            parsed.files.entries.get("manifest").map(String::as_str),
            Some("server.toml")
        );
    }

    #[test]
    fn capability_runtime_supported_is_required() {
        let src = r#"
            id = "mcp:linear"
            kind = "mcp"
            summary = "Linear MCP server."

            [files]
            manifest = "server.toml"

            [runtimes.codex]
        "#;

        let err = toml::from_str::<CapabilityManifest>(src).unwrap_err();
        assert!(err.to_string().contains("supported"));
    }
}
