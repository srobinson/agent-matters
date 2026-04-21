//! Profile manifest schema.
//!
//! Profiles compose capability ids, ordered instruction fragments, optional
//! scope metadata, and runtime enablement tables.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::domain::{CapabilityId, ProfileId, ProfileKind, RuntimeId, ScopeConstraints};

pub use crate::domain::ScopeEnforcement;

/// Authored `profiles/<id>/manifest.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileManifest {
    pub id: ProfileId,
    pub kind: ProfileKind,
    pub summary: String,
    pub capabilities: Vec<CapabilityId>,
    pub instructions: Vec<CapabilityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<ProfileScopeManifest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtimes: Option<ProfileRuntimesManifest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions_output: Option<InstructionsOutputManifest>,
}

/// Optional `[scope]` table. Validation of paths and repo constraints belongs
/// to the pure scope domain type.
pub type ProfileScopeManifest = ScopeConstraints;

/// `[runtimes]` table with an optional scalar `default` and explicit
/// per-runtime subtables.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileRuntimesManifest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<RuntimeId>,
    #[serde(flatten)]
    pub entries: BTreeMap<RuntimeId, ProfileRuntimeManifest>,
}

/// `[runtimes.<name>]` table. `enabled` has no default because runtime support
/// must be explicit in profile manifests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileRuntimeManifest {
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstructionsOutputManifest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub markers: Option<InstructionMarkers>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstructionMarkers {
    HtmlComments,
    TopNotice,
    None,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_manifest_parses_minimum_document() {
        let src = r#"
            id = "linear-triage"
            kind = "task"
            summary = "Triage Linear issues."
            capabilities = ["mcp:linear"]
            instructions = ["instruction:helioy-core"]
        "#;

        let parsed: ProfileManifest = toml::from_str(src).unwrap();

        assert_eq!(parsed.id.as_str(), "linear-triage");
        assert_eq!(parsed.kind, ProfileKind::Task);
        assert!(parsed.runtimes.is_none());
    }

    #[test]
    fn profile_runtime_enabled_is_required() {
        let src = r#"
            id = "linear-triage"
            kind = "task"
            summary = "Triage Linear issues."
            capabilities = ["mcp:linear"]
            instructions = ["instruction:helioy-core"]

            [runtimes.codex]
            model = "gpt-5.4"
        "#;

        let err = toml::from_str::<ProfileManifest>(src).unwrap_err();
        assert!(err.to_string().contains("enabled"));
    }

    #[test]
    fn runtimes_default_subtable_is_rejected() {
        let src = r#"
            id = "linear-triage"
            kind = "task"
            summary = "Triage Linear issues."
            capabilities = ["mcp:linear"]
            instructions = ["instruction:helioy-core"]

            [runtimes.default]

            [runtimes.codex]
            enabled = true
        "#;

        let err = toml::from_str::<ProfileManifest>(src).unwrap_err();
        assert!(err.to_string().contains("default"));
    }
}
