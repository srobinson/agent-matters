//! Typed deserialization schemas for the three MVP config surfaces:
//!
//! * `defaults/runtimes.toml` -> [`RuntimeDefaults`]
//! * `defaults/markers.toml`  -> [`Markers`]
//! * `~/.agent-matters/config.toml` -> [`UserConfig`]
//!
//! All fields are optional so missing files can produce `Default::default()`
//! values without losing round trip fidelity. The shared [`RuntimeSettings`]
//! type carries the per-runtime tables that appear in both the repo defaults
//! and user config surfaces. Profile manifests use a different, richer
//! runtime table described in `agent-matters-core::manifest::profile` and
//! are loaded through profile resolution, not this config path.
//!
//! Runtime precedence is not applied here. Callers load each surface into
//! its own struct and ALP-1932 composes them.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::domain::CapabilityKind;
use crate::manifest::InstructionMarkers;

/// Per-runtime settings shared between repo defaults and user config.
/// Fields are intentionally permissive for MVP; adapters read the subset
/// they need and ignore the rest.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeSettings {
    /// Optional model identifier (e.g. `"claude-sonnet-4.5"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Optional enabled flag. MVP does not require this field; profiles
    /// toggle runtime enablement in their own manifest tables.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

/// Contents of `defaults/runtimes.toml`. The runtime map is keyed by runtime
/// id (e.g. `"codex"`, `"claude"`). A missing file yields the empty default.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeDefaults {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub runtimes: BTreeMap<String, RuntimeSettings>,
}

/// Contents of `defaults/markers.toml`. Project markers are filenames that
/// identify a project root when walking upwards from a working directory
/// (e.g. `.git`, `Cargo.toml`). Scope and JIT resolution consume this list.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Markers {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub project_markers: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions_output: Option<InstructionOutputDefaults>,
}

/// Optional defaults for generated runtime instruction files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstructionOutputDefaults {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub markers: Option<InstructionMarkers>,
}

/// Policy that decides which external sources may import which capability
/// kinds. Runtime permissions are outside this MVP policy surface.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceTrustPolicy {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub sources: BTreeMap<String, SourceTrustRule>,
}

impl SourceTrustPolicy {
    pub fn conservative_default() -> Self {
        let mut sources = BTreeMap::new();
        sources.insert(
            "skills.sh".to_string(),
            SourceTrustRule {
                kinds: vec![CapabilityKind::Skill],
            },
        );
        Self { sources }
    }

    pub fn allows_source(&self, source: &str) -> bool {
        self.sources.contains_key(source)
    }

    pub fn allows_import(&self, source: &str, kind: CapabilityKind) -> bool {
        self.sources
            .get(source)
            .is_some_and(|rule| rule.kinds.contains(&kind))
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceTrustRule {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kinds: Vec<CapabilityKind>,
}

/// Contents of `~/.agent-matters/config.toml`. Only carries values the user
/// chooses to set; everything else falls through to repo defaults and then
/// runtime adapter defaults in the final precedence chain (ALP-1932).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserConfig {
    /// Default runtime to use when a `profiles use` invocation omits the
    /// `--runtime` flag and the profile does not pin a single runtime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_runtime: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions_output: Option<InstructionOutputDefaults>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub runtimes: BTreeMap<String, RuntimeSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_trust: Option<SourceTrustPolicy>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_defaults_empty_document_deserializes_to_default() {
        let parsed: RuntimeDefaults = toml::from_str("").unwrap();
        assert_eq!(parsed, RuntimeDefaults::default());
    }

    #[test]
    fn runtime_defaults_deserializes_runtime_map() {
        let src = r#"
            [runtimes.codex]
            model = "gpt-5.4"

            [runtimes.claude]
            model = "claude-sonnet-4.5"
            enabled = true
        "#;
        let parsed: RuntimeDefaults = toml::from_str(src).unwrap();
        assert_eq!(parsed.runtimes.len(), 2);
        assert_eq!(
            parsed.runtimes.get("codex").unwrap().model.as_deref(),
            Some("gpt-5.4")
        );
        assert_eq!(parsed.runtimes.get("claude").unwrap().enabled, Some(true));
    }

    #[test]
    fn markers_deserialize_and_round_trip() {
        let src = r#"
            project_markers = [".git", "Cargo.toml"]

            [instructions_output]
            markers = "top-notice"
        "#;
        let parsed: Markers = toml::from_str(src).unwrap();
        assert_eq!(
            parsed.project_markers,
            vec![".git".to_string(), "Cargo.toml".to_string()]
        );
        assert_eq!(
            parsed
                .instructions_output
                .as_ref()
                .and_then(|output| output.markers),
            Some(InstructionMarkers::TopNotice)
        );

        let reserialized = toml::to_string(&parsed).unwrap();
        let round_trip: Markers = toml::from_str(&reserialized).unwrap();
        assert_eq!(parsed, round_trip);
    }

    #[test]
    fn user_config_deserializes_default_runtime_only() {
        let src = r#"default_runtime = "codex""#;
        let parsed: UserConfig = toml::from_str(src).unwrap();
        assert_eq!(parsed.default_runtime.as_deref(), Some("codex"));
        assert!(parsed.runtimes.is_empty());
    }

    #[test]
    fn user_config_deserializes_instruction_output_defaults() {
        let src = r#"
            [instructions_output]
            markers = "none"
        "#;

        let parsed: UserConfig = toml::from_str(src).unwrap();

        assert_eq!(
            parsed
                .instructions_output
                .as_ref()
                .and_then(|output| output.markers),
            Some(InstructionMarkers::None)
        );
    }

    #[test]
    fn source_trust_policy_default_allows_skills_sh_skill_imports() {
        let policy = SourceTrustPolicy::conservative_default();

        assert!(policy.allows_source("skills.sh"));
        assert!(policy.allows_import("skills.sh", CapabilityKind::Skill));
        assert!(!policy.allows_import("skills.sh", CapabilityKind::Mcp));
        assert!(!policy.allows_source("unknown"));
    }

    #[test]
    fn user_config_deserializes_source_trust_policy() {
        let src = r#"
            [source_trust.sources."skills.sh"]
            kinds = ["skill"]

            [source_trust.sources."mcp-registry"]
            kinds = ["mcp"]
        "#;

        let parsed: UserConfig = toml::from_str(src).unwrap();
        let policy = parsed.source_trust.expect("source trust policy");

        assert!(policy.allows_import("skills.sh", CapabilityKind::Skill));
        assert!(policy.allows_import("mcp-registry", CapabilityKind::Mcp));
        assert!(!policy.allows_import("mcp-registry", CapabilityKind::Skill));
    }

    #[test]
    fn unknown_fields_are_rejected_with_actionable_error() {
        let src = r#"unexpected_key = true"#;
        let err = toml::from_str::<UserConfig>(src).unwrap_err();
        assert!(err.to_string().contains("unexpected_key"));
    }
}
